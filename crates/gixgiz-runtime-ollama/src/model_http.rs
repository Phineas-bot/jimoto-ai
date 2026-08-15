use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use bytes::Bytes;
use gixgiz_contracts::{ModelAcquisitionPhase, ModelAcquisitionProgress};
use gixgiz_runtime::ModelProgressSender;
use http_body_util::{BodyExt, Full};
use hyper::{
    Method, Request, Response,
    body::Incoming,
    client::conn::http1,
    header::{CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, HOST},
};
use hyper_util::rt::TokioIo;
use tokio::{net::TcpStream, task::JoinHandle, time};

use crate::{
    endpoint::ValidatedEndpoint,
    error::OllamaAdapterError,
    protocol::{
        PullEvent, PullLayerProgress, decode_pull_event, decode_readiness_response,
        encode_pull_request, encode_readiness_request,
    },
};

const PULL_ROUTE: &str = "/api/pull";
const GENERATE_ROUTE: &str = "/api/generate";

pub(crate) type ModelHttpFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, OllamaAdapterError>> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ModelHttpLimits {
    pub(crate) connect_timeout: Duration,
    pub(crate) idle_timeout: Duration,
    pub(crate) max_body_bytes: usize,
    pub(crate) max_line_bytes: usize,
    pub(crate) max_events: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PullOutcome {
    pub(crate) provider_integrity: bool,
}

pub(crate) trait OllamaModelHttpClient: Send + Sync {
    fn pull<'a>(
        &'a self,
        endpoint: &'a ValidatedEndpoint,
        provider_model_id: &'a str,
        progress: ModelProgressSender,
        limits: ModelHttpLimits,
    ) -> ModelHttpFuture<'a, PullOutcome>;

    fn readiness<'a>(
        &'a self,
        endpoint: &'a ValidatedEndpoint,
        provider_model_id: &'a str,
        limits: ModelHttpLimits,
    ) -> ModelHttpFuture<'a, ()>;
}

/// Direct HTTP/1.1 model transport with no proxy, redirect, DNS, or TLS surface.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HyperLoopbackModelHttpClient;

impl OllamaModelHttpClient for HyperLoopbackModelHttpClient {
    fn pull<'a>(
        &'a self,
        endpoint: &'a ValidatedEndpoint,
        provider_model_id: &'a str,
        progress: ModelProgressSender,
        limits: ModelHttpLimits,
    ) -> ModelHttpFuture<'a, PullOutcome> {
        Box::pin(async move {
            validate_pull_limits(limits)?;
            let request_body = encode_pull_request(provider_model_id)?;
            let (response, _connection_guard) =
                send_post(endpoint, PULL_ROUTE, request_body, limits.connect_timeout).await?;
            validate_response(&response, limits.max_body_bytes)?;
            parse_pull_body(response.into_body(), progress, limits).await
        })
    }

    fn readiness<'a>(
        &'a self,
        endpoint: &'a ValidatedEndpoint,
        provider_model_id: &'a str,
        limits: ModelHttpLimits,
    ) -> ModelHttpFuture<'a, ()> {
        Box::pin(async move {
            if limits.connect_timeout.is_zero()
                || limits.idle_timeout.is_zero()
                || limits.max_body_bytes == 0
            {
                return Err(OllamaAdapterError::TimedOut);
            }
            let request_body = encode_readiness_request(provider_model_id)?;
            let (response, _connection_guard) = send_post(
                endpoint,
                GENERATE_ROUTE,
                request_body,
                limits.connect_timeout,
            )
            .await?;
            validate_response(&response, limits.max_body_bytes)?;
            let body = read_bounded_body(
                response.into_body(),
                limits.idle_timeout,
                limits.max_body_bytes,
            )
            .await?;
            decode_readiness_response(&body)
        })
    }
}

async fn send_post(
    endpoint: &ValidatedEndpoint,
    route: &'static str,
    body: Vec<u8>,
    connect_timeout: Duration,
) -> Result<(Response<Incoming>, AbortOnDrop), OllamaAdapterError> {
    let (mut sender, guard) = time::timeout(connect_timeout, async {
        let stream = TcpStream::connect(endpoint.address())
            .await
            .map_err(|_| OllamaAdapterError::ConnectionFailed)?;
        let (sender, connection) = http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| OllamaAdapterError::ConnectionFailed)?;
        let guard = AbortOnDrop(tokio::spawn(async move {
            let _ = connection.await;
        }));
        Ok::<_, OllamaAdapterError>((sender, guard))
    })
    .await
    .map_err(|_| OllamaAdapterError::TimedOut)??;

    let request = Request::builder()
        .method(Method::POST)
        .uri(route)
        .header(HOST, endpoint.authority())
        .header(CONTENT_TYPE, "application/json")
        .header(CONNECTION, "close")
        .body(Full::new(Bytes::from(body)))
        .map_err(|_| OllamaAdapterError::InvalidEndpoint)?;
    let response = time::timeout(connect_timeout, sender.send_request(request))
        .await
        .map_err(|_| OllamaAdapterError::TimedOut)?
        .map_err(|_| OllamaAdapterError::ConnectionFailed)?;
    Ok((response, guard))
}

fn validate_response(
    response: &Response<Incoming>,
    max_body_bytes: usize,
) -> Result<(), OllamaAdapterError> {
    if !response.status().is_success() {
        return Err(OllamaAdapterError::HttpStatus {
            status: response.status().as_u16(),
        });
    }
    if response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > max_body_bytes)
    {
        return Err(OllamaAdapterError::ResponseTooLarge);
    }
    Ok(())
}

fn validate_pull_limits(limits: ModelHttpLimits) -> Result<(), OllamaAdapterError> {
    if limits.connect_timeout.is_zero()
        || limits.idle_timeout.is_zero()
        || limits.max_body_bytes == 0
        || limits.max_line_bytes == 0
        || limits.max_events == 0
    {
        return Err(OllamaAdapterError::TimedOut);
    }
    Ok(())
}

struct PullParseState {
    partial_line: Vec<u8>,
    events: usize,
    terminal: bool,
    provider_integrity: bool,
    layers: HashMap<String, LayerCounters>,
    aggregate_completed: u64,
    aggregate_total: u64,
}

#[derive(Clone, Copy)]
struct LayerCounters {
    completed: u64,
    total: u64,
}

impl PullParseState {
    fn apply_event(
        &mut self,
        event: PullEvent,
    ) -> Result<Option<ModelAcquisitionProgress>, OllamaAdapterError> {
        let phase = match event {
            PullEvent::Preparing => ModelAcquisitionPhase::Preparing,
            PullEvent::LayerProgress(layer) => {
                self.record_layer(layer)?;
                ModelAcquisitionPhase::Transferring
            }
            PullEvent::Verifying => {
                self.provider_integrity = true;
                ModelAcquisitionPhase::Verifying
            }
            PullEvent::Registering => ModelAcquisitionPhase::Registering,
            PullEvent::Success => {
                self.terminal = true;
                ModelAcquisitionPhase::Completed
            }
            PullEvent::Ignored => return Ok(None),
        };
        let completed_bytes = (!self.layers.is_empty()).then_some(self.aggregate_completed);
        // Core validates the monotonic discovered total against its approved expected size.
        let total_bytes =
            (phase == ModelAcquisitionPhase::Transferring).then_some(self.aggregate_total);
        Ok(Some(ModelAcquisitionProgress {
            phase,
            completed_bytes,
            total_bytes,
            progress_basis_points: None,
        }))
    }

    fn record_layer(&mut self, update: PullLayerProgress) -> Result<(), OllamaAdapterError> {
        if let Some(current) = self.layers.get_mut(&update.digest) {
            if current.total != update.total {
                return Err(OllamaAdapterError::InvalidResponse);
            }
            if update.completed < current.completed {
                return Err(OllamaAdapterError::InvalidResponse);
            }
            if update.completed == current.completed {
                return Ok(());
            }
            let delta = update.completed - current.completed;
            self.aggregate_completed = self
                .aggregate_completed
                .checked_add(delta)
                .filter(|value| *value <= i64::MAX as u64)
                .ok_or(OllamaAdapterError::InvalidResponse)?;
            current.completed = update.completed;
            return Ok(());
        }

        let aggregate_completed = self
            .aggregate_completed
            .checked_add(update.completed)
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or(OllamaAdapterError::InvalidResponse)?;
        let aggregate_total = self
            .aggregate_total
            .checked_add(update.total)
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or(OllamaAdapterError::InvalidResponse)?;
        self.aggregate_completed = aggregate_completed;
        self.aggregate_total = aggregate_total;
        self.layers.insert(
            update.digest,
            LayerCounters {
                completed: update.completed,
                total: update.total,
            },
        );
        Ok(())
    }
}

async fn parse_pull_body(
    mut body: Incoming,
    progress: ModelProgressSender,
    limits: ModelHttpLimits,
) -> Result<PullOutcome, OllamaAdapterError> {
    let mut state = PullParseState {
        partial_line: Vec::new(),
        events: 0,
        terminal: false,
        provider_integrity: false,
        layers: HashMap::new(),
        aggregate_completed: 0,
        aggregate_total: 0,
    };
    let mut total_bytes = 0_usize;
    loop {
        let frame = time::timeout(limits.idle_timeout, body.frame())
            .await
            .map_err(|_| OllamaAdapterError::TimedOut)?;
        let Some(frame) = frame else { break };
        let frame = frame.map_err(|_| OllamaAdapterError::ConnectionFailed)?;
        if let Ok(data) = frame.into_data() {
            total_bytes = total_bytes
                .checked_add(data.len())
                .ok_or(OllamaAdapterError::ResponseTooLarge)?;
            if total_bytes > limits.max_body_bytes {
                return Err(OllamaAdapterError::ResponseTooLarge);
            }
            consume_pull_bytes(&data, &progress, limits, &mut state)?;
        }
    }
    if !state.partial_line.iter().all(u8::is_ascii_whitespace) {
        let line = std::mem::take(&mut state.partial_line);
        consume_pull_line(&line, &progress, limits, &mut state)?;
    }
    if !state.terminal {
        return Err(OllamaAdapterError::ModelAcquisitionFailed);
    }
    Ok(PullOutcome {
        provider_integrity: state.provider_integrity,
    })
}

fn consume_pull_bytes(
    data: &[u8],
    progress: &ModelProgressSender,
    limits: ModelHttpLimits,
    state: &mut PullParseState,
) -> Result<(), OllamaAdapterError> {
    for part in data.split_inclusive(|byte| *byte == b'\n') {
        let is_complete = part.last() == Some(&b'\n');
        let content = if is_complete {
            &part[..part.len().saturating_sub(1)]
        } else {
            part
        };
        if state.partial_line.len().saturating_add(content.len()) > limits.max_line_bytes {
            return Err(OllamaAdapterError::ResponseTooLarge);
        }
        state.partial_line.extend_from_slice(content);
        if is_complete {
            if state.partial_line.last() == Some(&b'\r') {
                state.partial_line.pop();
            }
            if !state.partial_line.iter().all(u8::is_ascii_whitespace) {
                let line = std::mem::take(&mut state.partial_line);
                consume_pull_line(&line, progress, limits, state)?;
            } else {
                state.partial_line.clear();
            }
        }
    }
    Ok(())
}

fn consume_pull_line(
    line: &[u8],
    progress: &ModelProgressSender,
    limits: ModelHttpLimits,
    state: &mut PullParseState,
) -> Result<(), OllamaAdapterError> {
    state.events = state
        .events
        .checked_add(1)
        .ok_or(OllamaAdapterError::ResponseTooLarge)?;
    if state.events > limits.max_events {
        return Err(OllamaAdapterError::ResponseTooLarge);
    }
    let event = decode_pull_event(line)?;
    if state.terminal {
        return if matches!(event, PullEvent::Ignored) {
            Ok(())
        } else {
            Err(OllamaAdapterError::InvalidResponse)
        };
    }
    if let Some(update) = state.apply_event(event)? {
        let _ = progress.try_send(update);
    }
    Ok(())
}

async fn read_bounded_body(
    mut body: Incoming,
    idle_timeout: Duration,
    max_body_bytes: usize,
) -> Result<Vec<u8>, OllamaAdapterError> {
    let mut bytes = Vec::new();
    loop {
        let frame = time::timeout(idle_timeout, body.frame())
            .await
            .map_err(|_| OllamaAdapterError::TimedOut)?;
        let Some(frame) = frame else { break };
        let frame = frame.map_err(|_| OllamaAdapterError::ConnectionFailed)?;
        if let Ok(data) = frame.into_data() {
            if data.len() > max_body_bytes.saturating_sub(bytes.len()) {
                return Err(OllamaAdapterError::ResponseTooLarge);
            }
            bytes.extend_from_slice(&data);
        }
    }
    Ok(bytes)
}

struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use gixgiz_contracts::ModelAcquisitionProgress;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::mpsc,
    };

    use super::*;

    pub(crate) struct FakeModelHttpClient {
        pulls: Mutex<VecDeque<Result<PullOutcome, OllamaAdapterError>>>,
        readiness: Mutex<VecDeque<Result<(), OllamaAdapterError>>>,
        progress: Vec<ModelAcquisitionProgress>,
    }

    impl FakeModelHttpClient {
        pub(crate) fn new(
            pulls: impl IntoIterator<Item = Result<PullOutcome, OllamaAdapterError>>,
            readiness: impl IntoIterator<Item = Result<(), OllamaAdapterError>>,
            progress: Vec<ModelAcquisitionProgress>,
        ) -> Self {
            Self {
                pulls: Mutex::new(pulls.into_iter().collect()),
                readiness: Mutex::new(readiness.into_iter().collect()),
                progress,
            }
        }
    }

    impl OllamaModelHttpClient for FakeModelHttpClient {
        fn pull<'a>(
            &'a self,
            _endpoint: &'a ValidatedEndpoint,
            _provider_model_id: &'a str,
            progress: ModelProgressSender,
            _limits: ModelHttpLimits,
        ) -> ModelHttpFuture<'a, PullOutcome> {
            Box::pin(async move {
                for update in &self.progress {
                    let _ = progress.try_send(update.clone());
                }
                self.pulls
                    .lock()
                    .expect("fake pull lock")
                    .pop_front()
                    .expect("fake pull result")
            })
        }

        fn readiness<'a>(
            &'a self,
            _endpoint: &'a ValidatedEndpoint,
            _provider_model_id: &'a str,
            _limits: ModelHttpLimits,
        ) -> ModelHttpFuture<'a, ()> {
            Box::pin(async move {
                self.readiness
                    .lock()
                    .expect("fake readiness lock")
                    .pop_front()
                    .expect("fake readiness result")
            })
        }
    }

    fn limits() -> ModelHttpLimits {
        ModelHttpLimits {
            connect_timeout: Duration::from_secs(1),
            idle_timeout: Duration::from_secs(1),
            max_body_bytes: 64 * 1024,
            max_line_bytes: 16 * 1024,
            max_events: 32,
        }
    }

    async fn serve_once(route: &'static str, body: &'static [u8]) -> ValidatedEndpoint {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = vec![0_u8; 4096];
            let read = stream.read(&mut request).await.expect("read request");
            let text = String::from_utf8_lossy(&request[..read]);
            assert!(text.starts_with(&format!("POST {route} HTTP/1.1")));
            assert!(text.contains("\"insecure\":false") || route == GENERATE_ROUTE);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.expect("header");
            stream.write_all(body).await.expect("body");
        });
        ValidatedEndpoint::from_address(address)
    }

    async fn serve_stalled_headers() -> ValidatedEndpoint {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = vec![0_u8; 4096];
            let _ = stream.read(&mut request).await.expect("read request");
            time::sleep(Duration::from_secs(1)).await;
        });
        ValidatedEndpoint::from_address(address)
    }

    #[tokio::test]
    async fn direct_pull_client_aggregates_two_layers_into_monotonic_progress() {
        let endpoint = serve_once(
            PULL_ROUTE,
            concat!(
                "{\"status\":\"pulling manifest\"}\n",
                "{\"status\":\"pulling 0123456789ab\",\"digest\":\"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"total\":100,\"completed\":25}\n",
                "{\"status\":\"pulling 0123456789ab\",\"digest\":\"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"total\":100,\"completed\":25}\n",
                "{\"status\":\"pulling 0123456789ab\",\"digest\":\"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"total\":100,\"completed\":100}\n",
                "{\"status\":\"pulling fedcba987654\",\"digest\":\"sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210\",\"total\":300,\"completed\":0}\n",
                "{\"status\":\"future provider bookkeeping\",\"digest\":\"not-a-digest\",\"total\":1,\"completed\":2}\n",
                "{\"status\":\"pulling fedcba987654\",\"digest\":\"sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210\",\"total\":300,\"completed\":150}\n",
                "{\"status\":\"pulling fedcba987654\",\"digest\":\"sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210\",\"total\":300,\"completed\":300}\n",
                "{\"status\":\"verifying sha256 digest\"}\n",
                "{\"status\":\"writing manifest\"}\n",
                "{\"status\":\"removing any unused layers\"}\n",
                "{\"status\":\"success\"}\n",
                "{\"status\":\"future terminal bookkeeping\"}\n",
            )
            .as_bytes(),
        )
        .await;
        let (sender, mut receiver) = mpsc::channel(16);

        let outcome = HyperLoopbackModelHttpClient
            .pull(&endpoint, "qwen2.5:0.5b-instruct", sender, limits())
            .await
            .expect("pull stream succeeds");
        let mut updates = Vec::new();
        while let Ok(update) = receiver.try_recv() {
            updates.push(update);
        }

        assert!(outcome.provider_integrity);
        assert_eq!(updates.len(), 11);
        assert_eq!(updates[0].phase, ModelAcquisitionPhase::Preparing);
        assert_eq!(updates[0].completed_bytes, None);
        let completed: Vec<u64> = updates
            .iter()
            .filter_map(|update| update.completed_bytes)
            .collect();
        assert_eq!(
            completed,
            vec![25, 25, 100, 100, 250, 400, 400, 400, 400, 400]
        );
        assert!(completed.windows(2).all(|pair| pair[0] <= pair[1]));
        let totals: Vec<u64> = updates
            .iter()
            .filter_map(|update| update.total_bytes)
            .collect();
        assert_eq!(totals, vec![100, 100, 100, 400, 400, 400]);
        assert!(totals.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(
            updates
                .iter()
                .all(|update| update.progress_basis_points.is_none())
        );
        assert_eq!(
            updates.last(),
            Some(&ModelAcquisitionProgress {
                phase: ModelAcquisitionPhase::Completed,
                completed_bytes: Some(400),
                total_bytes: None,
                progress_basis_points: None,
            })
        );
    }

    #[tokio::test]
    async fn regressing_completed_counter_for_one_layer_fails_closed() {
        let endpoint = serve_once(
            PULL_ROUTE,
            concat!(
                "{\"status\":\"pulling 0123456789ab\",\"digest\":\"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"total\":100,\"completed\":50}\n",
                "{\"status\":\"pulling 0123456789ab\",\"digest\":\"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"total\":100,\"completed\":25}\n",
                "{\"status\":\"success\"}\n",
            )
            .as_bytes(),
        )
        .await;
        let (sender, _receiver) = mpsc::channel(8);

        let result = HyperLoopbackModelHttpClient
            .pull(&endpoint, "qwen2.5:0.5b-instruct", sender, limits())
            .await;

        assert_eq!(result, Err(OllamaAdapterError::InvalidResponse));
    }

    #[tokio::test]
    async fn changing_total_for_one_validated_layer_fails_closed() {
        let endpoint = serve_once(
            PULL_ROUTE,
            concat!(
                "{\"status\":\"pulling 0123456789ab\",\"digest\":\"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"total\":100,\"completed\":25}\n",
                "{\"status\":\"pulling 0123456789ab\",\"digest\":\"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"total\":101,\"completed\":50}\n",
                "{\"status\":\"success\"}\n",
            )
            .as_bytes(),
        )
        .await;
        let (sender, _receiver) = mpsc::channel(8);

        let result = HyperLoopbackModelHttpClient
            .pull(&endpoint, "qwen2.5:0.5b-instruct", sender, limits())
            .await;

        assert_eq!(result, Err(OllamaAdapterError::InvalidResponse));
    }

    #[tokio::test]
    async fn direct_readiness_client_discards_valid_generated_content() {
        let endpoint = serve_once(GENERATE_ROUTE, br#"{"response":"READY","done":true}"#).await;

        HyperLoopbackModelHttpClient
            .readiness(&endpoint, "qwen2.5:0.5b-instruct", limits())
            .await
            .expect("readiness succeeds");
    }

    #[tokio::test]
    async fn incomplete_pull_never_becomes_successful() {
        let endpoint = serve_once(PULL_ROUTE, b"{\"status\":\"pulling manifest\"}\n").await;
        let (sender, _receiver) = mpsc::channel(1);

        let result = HyperLoopbackModelHttpClient
            .pull(&endpoint, "qwen2.5:0.5b-instruct", sender, limits())
            .await;

        assert_eq!(result, Err(OllamaAdapterError::ModelAcquisitionFailed));
    }

    #[tokio::test]
    async fn stalled_response_headers_use_the_fixed_header_timeout() {
        let endpoint = serve_stalled_headers().await;
        let mut request_limits = limits();
        request_limits.connect_timeout = Duration::from_millis(20);

        let result = HyperLoopbackModelHttpClient
            .readiness(&endpoint, "qwen2.5:0.5b-instruct", request_limits)
            .await;

        assert_eq!(result, Err(OllamaAdapterError::TimedOut));
    }
}
