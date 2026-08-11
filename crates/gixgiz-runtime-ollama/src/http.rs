use std::{future::Future, pin::Pin, time::Duration};

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::{
    Method, Request,
    client::conn::http1,
    header::{CONTENT_LENGTH, HOST},
};
use hyper_util::rt::TokioIo;
use tokio::{net::TcpStream, task::JoinHandle, time};

use crate::{endpoint::ValidatedEndpoint, error::OllamaAdapterError};

const VERSION_ROUTE: &str = "/api/version";
const TAGS_ROUTE: &str = "/api/tags";

pub(crate) type HttpFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<u8>, OllamaAdapterError>> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OllamaRoute {
    Version,
    Tags,
}

impl OllamaRoute {
    const fn path(self) -> &'static str {
        match self {
            Self::Version => VERSION_ROUTE,
            Self::Tags => TAGS_ROUTE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HttpLimits {
    pub(crate) timeout: Duration,
    pub(crate) max_body_bytes: usize,
}

pub(crate) trait OllamaHttpClient: Send + Sync {
    fn get<'a>(
        &'a self,
        endpoint: &'a ValidatedEndpoint,
        route: OllamaRoute,
        limits: HttpLimits,
    ) -> HttpFuture<'a>;
}

/// Direct HTTP/1.1 transport with no proxy, redirect, DNS, or TLS surface.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HyperLoopbackHttpClient;

impl OllamaHttpClient for HyperLoopbackHttpClient {
    fn get<'a>(
        &'a self,
        endpoint: &'a ValidatedEndpoint,
        route: OllamaRoute,
        limits: HttpLimits,
    ) -> HttpFuture<'a> {
        Box::pin(async move {
            time::timeout(
                limits.timeout,
                get_inner(endpoint, route, limits.max_body_bytes),
            )
            .await
            .map_err(|_| OllamaAdapterError::TimedOut)?
        })
    }
}

async fn get_inner(
    endpoint: &ValidatedEndpoint,
    route: OllamaRoute,
    max_body_bytes: usize,
) -> Result<Vec<u8>, OllamaAdapterError> {
    let stream = TcpStream::connect(endpoint.address())
        .await
        .map_err(|_| OllamaAdapterError::ConnectionFailed)?;
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|_| OllamaAdapterError::ConnectionFailed)?;
    let _connection_guard = AbortOnDrop(tokio::spawn(async move {
        let _ = connection.await;
    }));

    let request = Request::builder()
        .method(Method::GET)
        .uri(route.path())
        .header(HOST, endpoint.authority())
        .body(Empty::<Bytes>::new())
        .map_err(|_| OllamaAdapterError::InvalidEndpoint)?;
    let response = sender
        .send_request(request)
        .await
        .map_err(|_| OllamaAdapterError::ConnectionFailed)?;

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

    let mut body = response.into_body();
    let mut bytes = Vec::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| OllamaAdapterError::ConnectionFailed)?;
        if let Ok(data) = frame.into_data() {
            let remaining = max_body_bytes.saturating_sub(bytes.len());
            if data.len() > remaining {
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

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    pub(crate) struct FakeHttpClient {
        responses: Mutex<VecDeque<Result<Vec<u8>, OllamaAdapterError>>>,
    }

    impl FakeHttpClient {
        pub(crate) fn new(
            responses: impl IntoIterator<Item = Result<Vec<u8>, OllamaAdapterError>>,
        ) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().collect()),
            }
        }
    }

    impl OllamaHttpClient for FakeHttpClient {
        fn get<'a>(
            &'a self,
            _endpoint: &'a ValidatedEndpoint,
            _route: OllamaRoute,
            _limits: HttpLimits,
        ) -> HttpFuture<'a> {
            Box::pin(async move {
                self.responses
                    .lock()
                    .expect("fake response lock")
                    .pop_front()
                    .expect("fake response exists")
            })
        }
    }

    async fn serve_once(body: &'static [u8]) -> ValidatedEndpoint {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let read = stream.read(&mut request).await.unwrap();
            assert!(
                String::from_utf8_lossy(&request[..read]).starts_with("GET /api/version HTTP/1.1")
            );
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(body).await.unwrap();
        });
        ValidatedEndpoint::from_address(address)
    }

    #[tokio::test]
    async fn direct_client_reads_documented_route_within_limit() {
        let body = br#"{"version":"0.12.6"}"#;
        let endpoint = serve_once(body).await;
        let result = HyperLoopbackHttpClient
            .get(
                &endpoint,
                OllamaRoute::Version,
                HttpLimits {
                    timeout: Duration::from_secs(1),
                    max_body_bytes: 128,
                },
            )
            .await
            .unwrap();

        assert_eq!(result, body);
    }

    #[tokio::test]
    async fn direct_client_rejects_oversized_body_before_parsing() {
        let endpoint = serve_once(br#"{"version":"0.12.6"}"#).await;
        let result = HyperLoopbackHttpClient
            .get(
                &endpoint,
                OllamaRoute::Version,
                HttpLimits {
                    timeout: Duration::from_secs(1),
                    max_body_bytes: 4,
                },
            )
            .await;

        assert_eq!(result, Err(OllamaAdapterError::ResponseTooLarge));
    }
}
