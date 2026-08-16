//! Bounded loopback streaming for the provider chat route.
//!
//! This module owns every Ollama chat detail: the route, the request shape, and
//! the newline-delimited response framing. It emits provider-neutral assistant
//! text only and never forwards provider status, thinking, or tool fields.

use std::{future::Future, pin::Pin, time::Duration};

use gixgiz_runtime::{ChatDeltaSender, RuntimeGenerationDelta};
use http_body_util::BodyExt;
use hyper::body::Incoming;
use tokio::time;

use crate::{
    endpoint::ValidatedEndpoint,
    error::OllamaAdapterError,
    model_http::{send_post, validate_response},
    protocol::{ChatEvent, ChatRequestMessage, decode_chat_event, encode_chat_request},
};

pub(crate) const CHAT_ROUTE: &str = "/api/chat";

pub(crate) type ChatHttpFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, OllamaAdapterError>> + Send + 'a>>;

/// Bounds applied to one streamed generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChatHttpLimits {
    pub(crate) connect_timeout: Duration,
    pub(crate) idle_timeout: Duration,
    pub(crate) max_line_bytes: usize,
    pub(crate) max_events: usize,
    pub(crate) max_output_bytes: usize,
    pub(crate) num_ctx: u32,
}

/// Terminal evidence for one validated streamed generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChatOutcome {
    pub(crate) emitted_bytes: usize,
}

pub(crate) trait OllamaChatHttpClient: Send + Sync {
    fn generate<'a>(
        &'a self,
        endpoint: &'a ValidatedEndpoint,
        provider_model_id: &'a str,
        messages: &'a [ChatRequestMessage<'a>],
        deltas: ChatDeltaSender,
        limits: ChatHttpLimits,
    ) -> ChatHttpFuture<'a, ChatOutcome>;
}

/// Direct HTTP/1.1 chat transport with no proxy, redirect, DNS, or TLS surface.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HyperLoopbackChatHttpClient;

impl OllamaChatHttpClient for HyperLoopbackChatHttpClient {
    fn generate<'a>(
        &'a self,
        endpoint: &'a ValidatedEndpoint,
        provider_model_id: &'a str,
        messages: &'a [ChatRequestMessage<'a>],
        deltas: ChatDeltaSender,
        limits: ChatHttpLimits,
    ) -> ChatHttpFuture<'a, ChatOutcome> {
        Box::pin(async move {
            validate_limits(limits)?;
            let request_body = encode_chat_request(provider_model_id, messages, limits.num_ctx)?;
            let (response, _connection_guard) =
                send_post(endpoint, CHAT_ROUTE, request_body, limits.connect_timeout).await?;
            validate_response(&response, limits.max_output_bytes.saturating_mul(4))?;
            stream_chat_body(response.into_body(), deltas, limits).await
        })
    }
}

struct ChatStreamState {
    partial_line: Vec<u8>,
    events: usize,
    emitted_bytes: usize,
    terminal: bool,
}

async fn stream_chat_body(
    mut body: Incoming,
    deltas: ChatDeltaSender,
    limits: ChatHttpLimits,
) -> Result<ChatOutcome, OllamaAdapterError> {
    let mut state = ChatStreamState {
        partial_line: Vec::new(),
        events: 0,
        emitted_bytes: 0,
        terminal: false,
    };

    loop {
        let frame = time::timeout(limits.idle_timeout, body.frame())
            .await
            .map_err(|_| OllamaAdapterError::TimedOut)?;
        let Some(frame) = frame else { break };
        let frame = frame.map_err(|_| OllamaAdapterError::ConnectionFailed)?;
        let Ok(data) = frame.into_data() else {
            continue;
        };

        for byte in data.as_ref() {
            if *byte == b'\n' {
                let line = std::mem::take(&mut state.partial_line);
                if consume_line(&line, &mut state, &deltas, limits).await? {
                    return Ok(ChatOutcome {
                        emitted_bytes: state.emitted_bytes,
                    });
                }
                continue;
            }
            if state.partial_line.len() >= limits.max_line_bytes {
                return Err(OllamaAdapterError::ResponseTooLarge);
            }
            state.partial_line.push(*byte);
        }
    }

    if !state.partial_line.is_empty() {
        let line = std::mem::take(&mut state.partial_line);
        if consume_line(&line, &mut state, &deltas, limits).await? {
            return Ok(ChatOutcome {
                emitted_bytes: state.emitted_bytes,
            });
        }
    }

    if state.terminal {
        return Ok(ChatOutcome {
            emitted_bytes: state.emitted_bytes,
        });
    }
    // The provider closed the stream without a validated terminal completion.
    Err(OllamaAdapterError::ConnectionFailed)
}

/// Consumes one framed line, returning whether the terminal event was observed.
async fn consume_line(
    line: &[u8],
    state: &mut ChatStreamState,
    deltas: &ChatDeltaSender,
    limits: ChatHttpLimits,
) -> Result<bool, OllamaAdapterError> {
    if line.iter().all(u8::is_ascii_whitespace) {
        return Ok(false);
    }
    state.events += 1;
    if state.events > limits.max_events {
        return Err(OllamaAdapterError::ResponseTooLarge);
    }

    match decode_chat_event(line)? {
        ChatEvent::Ignored => Ok(false),
        ChatEvent::Delta(text) => {
            emit(text, state, deltas, limits).await?;
            Ok(false)
        }
        ChatEvent::Done(trailing) => {
            if let Some(text) = trailing {
                emit(text, state, deltas, limits).await?;
            }
            state.terminal = true;
            Ok(true)
        }
    }
}

async fn emit(
    text: String,
    state: &mut ChatStreamState,
    deltas: &ChatDeltaSender,
    limits: ChatHttpLimits,
) -> Result<(), OllamaAdapterError> {
    let emitted = state
        .emitted_bytes
        .checked_add(text.len())
        .ok_or(OllamaAdapterError::ResponseTooLarge)?;
    if emitted > limits.max_output_bytes {
        return Err(OllamaAdapterError::ResponseTooLarge);
    }
    state.emitted_bytes = emitted;
    // A closed receiver means the caller stopped consuming; the connection
    // guard closes the provider request when this future is dropped.
    deltas
        .send(RuntimeGenerationDelta {
            text,
            emitted_bytes: emitted,
        })
        .await
        .map_err(|_| OllamaAdapterError::Cancelled)
}

fn validate_limits(limits: ChatHttpLimits) -> Result<(), OllamaAdapterError> {
    if limits.connect_timeout.is_zero()
        || limits.idle_timeout.is_zero()
        || limits.max_line_bytes == 0
        || limits.max_events == 0
        || limits.max_output_bytes == 0
        || limits.num_ctx == 0
    {
        return Err(OllamaAdapterError::TimedOut);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use gixgiz_runtime::CHAT_DELTA_CHANNEL_CAPACITY;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::mpsc,
        time,
    };

    use super::*;

    fn limits() -> ChatHttpLimits {
        ChatHttpLimits {
            connect_timeout: Duration::from_secs(2),
            idle_timeout: Duration::from_millis(300),
            max_line_bytes: 4096,
            max_events: 64,
            max_output_bytes: 1024,
            num_ctx: 4096,
        }
    }

    fn messages() -> Vec<ChatRequestMessage<'static>> {
        vec![ChatRequestMessage {
            role: "user",
            content: "hello",
        }]
    }

    /// Serves one bounded chat response, optionally leaving the stream unfinished.
    async fn serve(body: &'static str, close_early: bool) -> ValidatedEndpoint {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = vec![0_u8; 4096];
            let read = stream.read(&mut request).await.expect("read request");
            let text = String::from_utf8_lossy(&request[..read]);
            assert!(text.starts_with(&format!("POST {CHAT_ROUTE} HTTP/1.1")));
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.expect("header");
            stream.write_all(body.as_bytes()).await.expect("body");
            if !close_early {
                stream.flush().await.expect("flush");
            }
        });
        ValidatedEndpoint::from_address(address)
    }

    /// Accepts the request then stalls, exercising the inactivity bound.
    async fn serve_stalled() -> ValidatedEndpoint {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = vec![0_u8; 4096];
            let _ = stream.read(&mut request).await.expect("read request");
            let header = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n";
            stream.write_all(header.as_bytes()).await.expect("header");
            time::sleep(Duration::from_secs(2)).await;
        });
        ValidatedEndpoint::from_address(address)
    }

    async fn drain(
        endpoint: &ValidatedEndpoint,
        limits: ChatHttpLimits,
    ) -> (Result<ChatOutcome, OllamaAdapterError>, Vec<String>) {
        let (sender, mut receiver) = mpsc::channel(CHAT_DELTA_CHANNEL_CAPACITY);
        let client = HyperLoopbackChatHttpClient;
        let request = messages();
        let generation =
            client.generate(endpoint, "qwen2.5:0.5b-instruct", &request, sender, limits);
        let collector = tokio::spawn(async move {
            let mut deltas = Vec::new();
            while let Some(delta) = receiver.recv().await {
                deltas.push(delta.text);
            }
            deltas
        });
        let outcome = generation.await;
        let deltas = collector.await.expect("collector joins");
        (outcome, deltas)
    }

    #[tokio::test]
    async fn ordered_chunks_stream_and_reach_terminal_completion() {
        let endpoint = serve(
            concat!(
                "{\"message\":{\"content\":\"Hel\"},\"done\":false}\n",
                "{\"message\":{\"content\":\"lo \"},\"done\":false}\n",
                "{\"message\":{\"content\":\"there\"},\"done\":true}\n",
            ),
            false,
        )
        .await;

        let (outcome, deltas) = drain(&endpoint, limits()).await;

        assert_eq!(deltas, vec!["Hel", "lo ", "there"]);
        assert_eq!(
            outcome.expect("generation completes").emitted_bytes,
            "Hello there".len()
        );
    }

    #[tokio::test]
    async fn a_stream_without_a_terminal_chunk_is_a_disconnect() {
        let endpoint = serve(
            "{\"message\":{\"content\":\"partial\"},\"done\":false}\n",
            true,
        )
        .await;

        let (outcome, deltas) = drain(&endpoint, limits()).await;

        assert_eq!(deltas, vec!["partial"]);
        assert_eq!(outcome, Err(OllamaAdapterError::ConnectionFailed));
    }

    #[tokio::test]
    async fn a_malformed_chunk_fails_closed_mid_stream() {
        let endpoint = serve(
            concat!(
                "{\"message\":{\"content\":\"ok\"},\"done\":false}\n",
                "not json\n",
                "{\"message\":{\"content\":\"never\"},\"done\":true}\n",
            ),
            false,
        )
        .await;

        let (outcome, deltas) = drain(&endpoint, limits()).await;

        assert_eq!(deltas, vec!["ok"]);
        assert_eq!(outcome, Err(OllamaAdapterError::InvalidResponse));
    }

    #[tokio::test]
    async fn a_provider_reported_error_stops_the_generation() {
        let endpoint = serve("{\"error\":\"runner failed\"}\n", false).await;

        let (outcome, deltas) = drain(&endpoint, limits()).await;

        assert!(deltas.is_empty());
        assert_eq!(outcome, Err(OllamaAdapterError::GenerationFailed));
    }

    #[tokio::test]
    async fn output_beyond_the_bound_stops_the_generation() {
        let endpoint = serve(
            concat!(
                "{\"message\":{\"content\":\"aaaaaaaaaa\"},\"done\":false}\n",
                "{\"message\":{\"content\":\"bbbbbbbbbb\"},\"done\":true}\n",
            ),
            false,
        )
        .await;
        let mut bounded = limits();
        bounded.max_output_bytes = 15;

        let (outcome, _deltas) = drain(&endpoint, bounded).await;

        assert_eq!(outcome, Err(OllamaAdapterError::ResponseTooLarge));
    }

    #[tokio::test]
    async fn an_idle_provider_hits_the_inactivity_bound() {
        let endpoint = serve_stalled().await;

        let (outcome, deltas) = drain(&endpoint, limits()).await;

        assert!(deltas.is_empty());
        assert_eq!(outcome, Err(OllamaAdapterError::TimedOut));
    }

    #[tokio::test]
    async fn a_dropped_consumer_stops_the_generation_cooperatively() {
        let endpoint = serve(
            concat!(
                "{\"message\":{\"content\":\"one\"},\"done\":false}\n",
                "{\"message\":{\"content\":\"two\"},\"done\":true}\n",
            ),
            false,
        )
        .await;
        let (sender, receiver) = mpsc::channel(1);
        drop(receiver);
        let client = HyperLoopbackChatHttpClient;
        let request = messages();

        let outcome = client
            .generate(
                &endpoint,
                "qwen2.5:0.5b-instruct",
                &request,
                sender,
                limits(),
            )
            .await;

        assert_eq!(outcome, Err(OllamaAdapterError::Cancelled));
    }

    #[tokio::test]
    async fn zero_bounds_are_refused_before_any_connection() {
        let endpoint = ValidatedEndpoint::from_address("127.0.0.1:9".parse().expect("address"));
        let (sender, _receiver) = mpsc::channel(1);
        let mut invalid = limits();
        invalid.max_output_bytes = 0;
        let client = HyperLoopbackChatHttpClient;
        let request = messages();

        let outcome = client
            .generate(
                &endpoint,
                "qwen2.5:0.5b-instruct",
                &request,
                sender,
                invalid,
            )
            .await;

        assert_eq!(outcome, Err(OllamaAdapterError::TimedOut));
    }
}
