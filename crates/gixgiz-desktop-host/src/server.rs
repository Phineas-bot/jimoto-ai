use std::{convert::Infallible, net::SocketAddr, str::FromStr, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Extension, Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, post},
};
use gixgiz_contracts::{
    CancelOperationRequest, CancelOperationResponse, ClientHello, CoreHello, CorrelationId,
    ErrorCategory, HardwareScanEvent, HardwareScanStartRequest, HardwareScanStartResponse,
    HealthRequest, HealthResponse, InstanceId, OperationId, PROTOCOL_VERSION, PlatformStatus,
    RecommendationRequest, RecommendationResponse, RecoveryAction, RecoveryGuidance, RequestId,
    SafeErrorPayload, ShutdownRequest, ShutdownResponse, TestOperationStartRequest,
    TestOperationStartResponse, TransportCapability,
};
use gixgiz_core::{CapabilityEngine, CoreError, HardwareScanner, OperationContext};
use tokio::sync::{Semaphore, mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    HostError,
    bootstrap::BearerToken,
    hardware_scans::HardwareScanRegistry,
    operations::{OperationError, OperationRegistry},
};

const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_CONCURRENT_REQUESTS: usize = 16;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CORRELATION_HEADER: &str = "x-gixgiz-correlation-id";
const REQUEST_HEADER: &str = "x-gixgiz-request-id";

#[derive(Clone)]
struct AppState {
    token: BearerToken,
    instance_id: InstanceId,
    status: PlatformStatus,
    handshaken: Arc<std::sync::atomic::AtomicBool>,
    operations: OperationRegistry,
    hardware_scans: HardwareScanRegistry,
    capability_engine: CapabilityEngine,
    request_slots: Arc<Semaphore>,
    shutdown: ShutdownHandle,
}

#[derive(Clone, Copy)]
struct BoundaryIds {
    correlation_id: CorrelationId,
    request_id: RequestId,
}

/// Signal used by authenticated shutdown and supervisor-pipe loss.
#[derive(Clone)]
pub(crate) struct ShutdownHandle {
    sender: watch::Sender<bool>,
}

impl ShutdownHandle {
    pub(crate) fn request(&self) {
        let _ = self.sender.send(true);
    }
}

/// Bound loopback listener plus the internal transport router.
pub(crate) struct SidecarHost {
    listener: tokio::net::TcpListener,
    router: Router,
    local_addr: SocketAddr,
    shutdown_receiver: watch::Receiver<bool>,
    shutdown: ShutdownHandle,
}

impl SidecarHost {
    pub(crate) async fn bind(
        token: BearerToken,
        instance_id: InstanceId,
        status: PlatformStatus,
        hardware_scanner: HardwareScanner,
    ) -> Result<Self, HostError> {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(HostError::Bind)?;
        let local_addr = listener.local_addr().map_err(HostError::Bind)?;
        if local_addr.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) {
            return Err(HostError::Bind(std::io::Error::other(
                "listener did not bind to IPv4 loopback",
            )));
        }

        let (shutdown_sender, shutdown_receiver) = watch::channel(false);
        let shutdown = ShutdownHandle {
            sender: shutdown_sender,
        };
        let state = AppState {
            token,
            instance_id,
            status,
            handshaken: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            operations: OperationRegistry::default(),
            hardware_scans: HardwareScanRegistry::new(hardware_scanner),
            capability_engine: CapabilityEngine::v0_1(),
            request_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            shutdown: shutdown.clone(),
        };

        Ok(Self {
            listener,
            router: build_router(state),
            local_addr,
            shutdown_receiver,
            shutdown,
        })
    }

    pub(crate) const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub(crate) fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown.clone()
    }

    pub(crate) async fn run(self) -> Result<(), HostError> {
        let Self {
            listener,
            router,
            mut shutdown_receiver,
            ..
        } = self;
        let shutdown = async move {
            while !*shutdown_receiver.borrow() {
                if shutdown_receiver.changed().await.is_err() {
                    return;
                }
            }
        };

        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(HostError::Serve)
    }
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/internal/v1/handshake", post(handshake))
        .route("/internal/v1/health", post(health))
        .route("/internal/v1/test-operations", post(start_operation))
        .route(
            "/internal/v1/test-operations/{operation_id}/events",
            get(operation_events),
        )
        .route(
            "/internal/v1/test-operations/{operation_id}/cancel",
            post(cancel_operation),
        )
        .route("/internal/v1/hardware-scans", post(start_hardware_scan))
        .route(
            "/internal/v1/hardware-scans/{operation_id}/events",
            get(hardware_scan_events),
        )
        .route(
            "/internal/v1/hardware-scans/{operation_id}/cancel",
            post(cancel_hardware_scan),
        )
        .route("/internal/v1/recommendations", post(recommendation))
        .route("/internal/v1/shutdown", post(shutdown))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), request_guard))
        .with_state(state)
}

async fn request_guard(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let ids = ids_from_headers(request.headers());
    request.extensions_mut().insert(ids);

    let authenticated = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| state.token.matches_authorization(value));
    if !authenticated {
        return ApiFailure::new(
            StatusCode::UNAUTHORIZED,
            ErrorCategory::PermissionDenied,
            "transport.authentication_required",
            "The desktop could not authenticate with the local core.",
            RecoveryAction::Restart,
            "Restart GixGiz to create a new authenticated core session.",
            ids,
        )
        .into_response();
    }

    if request.headers().contains_key(header::ORIGIN) {
        return ApiFailure::new(
            StatusCode::FORBIDDEN,
            ErrorCategory::PermissionDenied,
            "transport.browser_origin_rejected",
            "Browser-origin requests are not accepted by the local core.",
            RecoveryAction::NoAction,
            "Use the installed GixGiz desktop application.",
            ids,
        )
        .into_response();
    }

    if request.method() == Method::POST && !has_json_content_type(request.headers()) {
        return ApiFailure::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCategory::InvalidInput,
            "transport.json_content_type_required",
            "The local core requires a JSON request body.",
            RecoveryAction::Retry,
            "Retry with the application/json content type.",
            ids,
        )
        .into_response();
    }

    if request.uri().path() != "/internal/v1/handshake"
        && !state.handshaken.load(std::sync::atomic::Ordering::Acquire)
    {
        return ApiFailure::new(
            StatusCode::PRECONDITION_REQUIRED,
            ErrorCategory::Conflict,
            "transport.handshake_required",
            "The desktop must complete a compatible handshake first.",
            RecoveryAction::Restart,
            "Restart GixGiz and complete the core handshake.",
            ids,
        )
        .into_response();
    }

    let Ok(permit) = state.request_slots.clone().try_acquire_owned() else {
        return ApiFailure::new(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCategory::ResourceExhausted,
            "transport.concurrent_request_limit",
            "The local core is handling the maximum number of requests.",
            RecoveryAction::Retry,
            "Wait briefly, then retry the request.",
            ids,
        )
        .into_response();
    };

    let response = tokio::time::timeout(REQUEST_TIMEOUT, next.run(request)).await;
    drop(permit);
    match response {
        Ok(response) => response,
        Err(_) => ApiFailure::new(
            StatusCode::GATEWAY_TIMEOUT,
            ErrorCategory::TimedOut,
            "transport.request_timed_out",
            "The local core did not finish the request in time.",
            RecoveryAction::Retry,
            "Retry the request.",
            ids,
        )
        .into_response(),
    }
}

async fn handshake(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<ClientHello>, JsonRejection>,
) -> Result<Json<CoreHello>, ApiFailure> {
    let hello = parse_json(payload, ids)?.0;
    ensure_ids(hello.correlation_id, hello.request_id, ids)?;
    let selected_protocol =
        negotiate_protocol(hello.protocol_min, hello.protocol_max).ok_or_else(|| {
            ApiFailure::new(
                StatusCode::CONFLICT,
                ErrorCategory::IncompatibleVersion,
                "transport.protocol_incompatible",
                "The desktop and local core use incompatible protocol versions.",
                RecoveryAction::Restart,
                "Install matching GixGiz desktop and core versions, then restart.",
                ids,
            )
        })?;

    state
        .handshaken
        .store(true, std::sync::atomic::Ordering::Release);
    Ok(Json(CoreHello {
        application: state.status.application.clone(),
        selected_protocol,
        supported_capabilities: supported_capabilities(),
        readiness: state.status.readiness.clone(),
        instance_id: state.instance_id,
        correlation_id: hello.correlation_id,
        request_id: hello.request_id,
    }))
}

async fn health(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<HealthRequest>, JsonRejection>,
) -> Result<Json<HealthResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    Ok(Json(HealthResponse {
        status: state.status.clone(),
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn start_operation(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<TestOperationStartRequest>, JsonRejection>,
) -> Result<Json<TestOperationStartResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let operation_id = state
        .operations
        .start(request.correlation_id)
        .map_err(|error| operation_failure(error, ids))?;
    Ok(Json(TestOperationStartResponse {
        operation_id,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn operation_events(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(operation_id): Path<String>,
) -> Result<Sse<ReceiverStream<Result<Event, Infallible>>>, ApiFailure> {
    let operation_id =
        OperationId::from_str(&operation_id).map_err(|_| invalid_operation_id(ids))?;
    let mut subscription = state
        .operations
        .subscribe(operation_id)
        .map_err(|error| operation_failure(error, ids))?;
    let (sender, receiver) = mpsc::channel(16);

    tokio::spawn(async move {
        for event in subscription.replay {
            let terminal = event.terminal_state.is_some();
            if send_event(&sender, event).await.is_err() || terminal {
                return;
            }
        }
        if subscription.terminal {
            return;
        }
        loop {
            match subscription.receiver.recv().await {
                Ok(event) => {
                    let terminal = event.terminal_state.is_some();
                    if send_event(&sender, event).await.is_err() || terminal {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(receiver)))
}

async fn cancel_operation(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(operation_id): Path<String>,
    payload: Result<Json<CancelOperationRequest>, JsonRejection>,
) -> Result<Json<CancelOperationResponse>, ApiFailure> {
    let operation_id =
        OperationId::from_str(&operation_id).map_err(|_| invalid_operation_id(ids))?;
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let accepted = state
        .operations
        .cancel(operation_id, request.correlation_id)
        .map_err(|error| operation_failure(error, ids))?;
    Ok(Json(CancelOperationResponse {
        operation_id,
        accepted,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn start_hardware_scan(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<HardwareScanStartRequest>, JsonRejection>,
) -> Result<Json<HardwareScanStartResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let operation_id = state
        .hardware_scans
        .start(request.correlation_id, request.request_id)
        .map_err(|error| operation_failure(error, ids))?;
    Ok(Json(HardwareScanStartResponse {
        operation_id,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn hardware_scan_events(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(operation_id): Path<String>,
) -> Result<Sse<ReceiverStream<Result<Event, Infallible>>>, ApiFailure> {
    let operation_id =
        OperationId::from_str(&operation_id).map_err(|_| invalid_operation_id(ids))?;
    let mut subscription = state
        .hardware_scans
        .subscribe(operation_id, ids.correlation_id)
        .map_err(|error| operation_failure(error, ids))?;
    let (sender, receiver) = mpsc::channel(8);

    tokio::spawn(async move {
        for event in subscription.replay {
            let terminal = event.terminal_state.is_some();
            if send_hardware_event(&sender, event).await.is_err() || terminal {
                return;
            }
        }
        if subscription.terminal {
            return;
        }
        loop {
            match subscription.receiver.recv().await {
                Ok(event) => {
                    let terminal = event.terminal_state.is_some();
                    if send_hardware_event(&sender, event).await.is_err() || terminal {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(receiver)))
}

async fn cancel_hardware_scan(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(operation_id): Path<String>,
    payload: Result<Json<CancelOperationRequest>, JsonRejection>,
) -> Result<Json<CancelOperationResponse>, ApiFailure> {
    let operation_id =
        OperationId::from_str(&operation_id).map_err(|_| invalid_operation_id(ids))?;
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let accepted = state
        .hardware_scans
        .cancel(operation_id, request.correlation_id)
        .map_err(|error| operation_failure(error, ids))?;
    Ok(Json(CancelOperationResponse {
        operation_id,
        accepted,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn recommendation(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<RecommendationRequest>, JsonRejection>,
) -> Result<Json<RecommendationResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let report = state
        .capability_engine
        .recommend(&request.machine_profile, request.preferences)
        .map_err(|error| capability_failure(error, ids))?;
    Ok(Json(RecommendationResponse {
        report,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn shutdown(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<ShutdownRequest>, JsonRejection>,
) -> Result<Json<ShutdownResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    state.shutdown.request();
    Ok(Json(ShutdownResponse {
        accepted: true,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn send_event(
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    event: gixgiz_contracts::TestOperationEvent,
) -> Result<(), ()> {
    let sequence = event.sequence.to_string();
    let data = serde_json::to_string(&event).map_err(|_| ())?;
    sender
        .send(Ok(Event::default()
            .event("test_operation")
            .id(sequence)
            .data(data)))
        .await
        .map_err(|_| ())
}

async fn send_hardware_event(
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    event: HardwareScanEvent,
) -> Result<(), ()> {
    let sequence = event.sequence.to_string();
    let data = serde_json::to_string(&event).map_err(|_| ())?;
    sender
        .send(Ok(Event::default()
            .event("hardware_scan")
            .id(sequence)
            .data(data)))
        .await
        .map_err(|_| ())
}

fn parse_json<T>(
    payload: Result<Json<T>, JsonRejection>,
    ids: BoundaryIds,
) -> Result<Json<T>, ApiFailure> {
    payload.map_err(|_| {
        ApiFailure::new(
            StatusCode::BAD_REQUEST,
            ErrorCategory::InvalidInput,
            "transport.invalid_json_body",
            "The local core could not read the request body.",
            RecoveryAction::Retry,
            "Retry with a valid bounded JSON request.",
            ids,
        )
    })
}

fn ensure_ids(
    correlation_id: CorrelationId,
    request_id: RequestId,
    expected: BoundaryIds,
) -> Result<(), ApiFailure> {
    if correlation_id == expected.correlation_id && request_id == expected.request_id {
        return Ok(());
    }
    Err(ApiFailure::new(
        StatusCode::BAD_REQUEST,
        ErrorCategory::InvalidInput,
        "transport.identifier_mismatch",
        "The request identifiers did not match the transport headers.",
        RecoveryAction::Retry,
        "Retry the request with matching identifiers.",
        expected,
    ))
}

fn negotiate_protocol(client_min: u32, client_max: u32) -> Option<u32> {
    if client_min == 0
        || client_min > client_max
        || PROTOCOL_VERSION < client_min
        || PROTOCOL_VERSION > client_max
    {
        None
    } else {
        Some(PROTOCOL_VERSION)
    }
}

fn supported_capabilities() -> Vec<TransportCapability> {
    vec![
        TransportCapability::Health,
        TransportCapability::TestOperationEvents,
        TransportCapability::HardwareScan,
        TransportCapability::CapabilityRecommendation,
        TransportCapability::Cancellation,
        TransportCapability::Shutdown,
    ]
}

fn ids_from_headers(headers: &HeaderMap) -> BoundaryIds {
    let correlation_id = headers
        .get(CORRELATION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| CorrelationId::from_str(value).ok())
        .unwrap_or_default();
    let request_id = headers
        .get(REQUEST_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| RequestId::from_str(value).ok())
        .unwrap_or_default();
    BoundaryIds {
        correlation_id,
        request_id,
    }
}

fn has_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}

fn invalid_operation_id(ids: BoundaryIds) -> ApiFailure {
    ApiFailure::new(
        StatusCode::BAD_REQUEST,
        ErrorCategory::InvalidInput,
        "transport.invalid_operation_id",
        "The operation identifier was invalid.",
        RecoveryAction::NoAction,
        "Start a new operation before retrying.",
        ids,
    )
}

fn operation_failure(error: OperationError, ids: BoundaryIds) -> ApiFailure {
    match error {
        OperationError::NotFound => ApiFailure::new(
            StatusCode::NOT_FOUND,
            ErrorCategory::Unavailable,
            "transport.operation_not_found",
            "The requested foundation operation is not available.",
            RecoveryAction::Retry,
            "Start a new foundation operation.",
            ids,
        ),
        OperationError::CorrelationMismatch => ApiFailure::new(
            StatusCode::BAD_REQUEST,
            ErrorCategory::InvalidInput,
            "transport.operation_correlation_mismatch",
            "The cancellation correlation identifier did not match the operation.",
            RecoveryAction::NoAction,
            "Use the correlation identifier returned when the operation started.",
            ids,
        ),
        OperationError::Busy => ApiFailure::new(
            StatusCode::CONFLICT,
            ErrorCategory::Conflict,
            "hardware.scan_in_progress",
            "A hardware evidence scan is already in progress.",
            RecoveryAction::Retry,
            "Wait for the current scan to finish or cancel it before retrying.",
            ids,
        ),
        OperationError::Internal => ApiFailure::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCategory::Internal,
            "transport.operation_state_unavailable",
            "The local core could not access the foundation operation state.",
            RecoveryAction::Restart,
            "Restart GixGiz and retry.",
            ids,
        ),
    }
}

fn capability_failure(error: CoreError, ids: BoundaryIds) -> ApiFailure {
    let context = OperationContext::new(ids.correlation_id, ids.request_id);
    let payload = error.to_safe_payload(&context);
    let status = match payload.category {
        ErrorCategory::InvalidInput => StatusCode::BAD_REQUEST,
        ErrorCategory::IncompatibleVersion => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiFailure {
        status,
        payload: Box::new(payload),
    }
}

struct ApiFailure {
    status: StatusCode,
    payload: Box<SafeErrorPayload>,
}

impl ApiFailure {
    #[allow(clippy::too_many_arguments)]
    fn new(
        status: StatusCode,
        category: ErrorCategory,
        code: &str,
        message: &str,
        recovery_action: RecoveryAction,
        recovery_message: &str,
        ids: BoundaryIds,
    ) -> Self {
        Self {
            status,
            payload: Box::new(SafeErrorPayload::new(
                category,
                code,
                message,
                RecoveryGuidance {
                    action: recovery_action,
                    message: recovery_message.to_owned(),
                },
                ids.correlation_id,
                ids.request_id,
            )),
        }
    }
}

impl IntoResponse for ApiFailure {
    fn into_response(self) -> Response {
        let correlation = self.payload.correlation_id.to_string();
        let request = self.payload.request_id.to_string();
        let mut response = (self.status, Json(*self.payload)).into_response();
        if let Ok(value) = HeaderValue::from_str(&correlation) {
            response.headers_mut().insert(CORRELATION_HEADER, value);
        }
        if let Ok(value) = HeaderValue::from_str(&request) {
            response.headers_mut().insert(REQUEST_HEADER, value);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use axum::http::Request;
    use gixgiz_contracts::{
        AccelerationEvidence, AccelerationKind, ArchitectureEvidence, CpuEvidence,
        EvidenceConfidence, EvidenceMetadata, EvidenceSource, GpuCollectionEvidence,
        MachineArchitecture, MachineProfile, MachineProfileCompleteness, OperatingSystemEvidence,
        PhysicalMemoryEvidence, PreferencePriority, ServiceHealthStatus, ServiceRequirement,
        StorageEvidence, StorageLocation, StorageMediaEvidence, StorageMediaKind, StringEvidence,
        U32Evidence, U64Evidence, UserPreferenceProfile, WorkloadTier,
    };
    use gixgiz_core::{
        CollectedHardwareEvidence, CoreError, HardwareProvider, OperationContext, PlatformCore,
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;

    const TOKEN: &str = "abababababababababababababababababababababababababababababababab";

    struct UnavailableProvider;

    impl HardwareProvider for UnavailableProvider {
        fn collect(
            &self,
            _context: &OperationContext,
        ) -> Result<CollectedHardwareEvidence, CoreError> {
            Err(CoreError::HardwareProviderUnavailable)
        }
    }

    fn test_hardware_scanner() -> HardwareScanner {
        HardwareScanner::new(Arc::new(UnavailableProvider))
    }

    fn state_with_status(status: PlatformStatus) -> AppState {
        let (shutdown_sender, _) = watch::channel(false);
        AppState {
            token: BearerToken::parse(TOKEN.to_owned()).expect("fixed token is valid"),
            instance_id: InstanceId::new(),
            status,
            handshaken: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            operations: OperationRegistry::default(),
            hardware_scans: HardwareScanRegistry::new(test_hardware_scanner()),
            capability_engine: CapabilityEngine::v0_1(),
            request_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            shutdown: ShutdownHandle {
                sender: shutdown_sender,
            },
        }
    }

    fn test_state() -> AppState {
        let context = OperationContext::generated();
        let mut core = PlatformCore::new(Vec::new());
        let status = core
            .start(&context)
            .expect("core starts with fake-free status");
        state_with_status(status)
    }

    fn hello(protocol_min: u32, protocol_max: u32) -> ClientHello {
        ClientHello {
            client_name: "GixGiz desktop".to_owned(),
            client_version: "0.1.0".to_owned(),
            protocol_min,
            protocol_max,
            requested_capabilities: vec![TransportCapability::Health],
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        }
    }

    fn recommendation_request() -> RecommendationRequest {
        RecommendationRequest {
            machine_profile: test_profile(),
            preferences: UserPreferenceProfile {
                workload: WorkloadTier::GeneralText,
                priority: PreferencePriority::Balanced,
                include_optional_larger: true,
            },
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        }
    }

    fn test_profile() -> MachineProfile {
        const GIB: u64 = 1024 * 1024 * 1024;
        let metadata =
            EvidenceMetadata::available(EvidenceSource::WindowsCim, EvidenceConfidence::High);
        let text = |value: &str| StringEvidence {
            value: Some(value.to_owned()),
            metadata: metadata.clone(),
        };
        let u64_value = |value| U64Evidence {
            value: Some(value),
            metadata: metadata.clone(),
        };

        MachineProfile {
            schema_version: gixgiz_contracts::MACHINE_PROFILE_SCHEMA_VERSION,
            scan_id: OperationId::new(),
            correlation_id: CorrelationId::new(),
            scanned_at_unix_ms: 1_725_000_000_000,
            completeness: MachineProfileCompleteness::Complete,
            operating_system: OperatingSystemEvidence {
                name: text("Windows 11"),
                version: text("10.0"),
                build: text("26100"),
                architecture: ArchitectureEvidence {
                    value: Some(MachineArchitecture::X86_64),
                    metadata: metadata.clone(),
                },
            },
            cpu: CpuEvidence {
                name: text("Fixture CPU"),
                vendor: text("Fixture vendor"),
                physical_core_count: U32Evidence {
                    value: Some(4),
                    metadata: metadata.clone(),
                },
                logical_core_count: U32Evidence {
                    value: Some(8),
                    metadata: metadata.clone(),
                },
            },
            physical_memory: PhysicalMemoryEvidence {
                total_bytes: u64_value(16 * GIB),
                available_bytes: u64_value(10 * GIB),
            },
            gpus: GpuCollectionEvidence {
                devices: Vec::new(),
                metadata: metadata.clone(),
            },
            acceleration: vec![AccelerationEvidence {
                kind: AccelerationKind::DirectMl,
                supported: Some(false),
                metadata: metadata.clone(),
            }],
            storage: StorageEvidence {
                location: StorageLocation::ApplicationData,
                capacity_bytes: u64_value(200 * GIB),
                free_bytes: u64_value(30 * GIB),
                filesystem: text("NTFS"),
                media_kind: StorageMediaEvidence {
                    value: Some(StorageMediaKind::Fixed),
                    metadata,
                },
            },
        }
    }

    fn json_request<T: serde::Serialize>(
        path: &str,
        token: Option<&str>,
        payload: &T,
    ) -> Request<Body> {
        let value = serde_json::to_vec(payload).expect("test payload serializes");
        let correlation = serde_json::to_value(payload)
            .ok()
            .and_then(|value| {
                value
                    .get("correlation_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| CorrelationId::new().to_string());
        let request_id = serde_json::to_value(payload)
            .ok()
            .and_then(|value| {
                value
                    .get("request_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| RequestId::new().to_string());
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(CORRELATION_HEADER, correlation)
            .header(REQUEST_HEADER, request_id);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::from(value)).expect("request builds")
    }

    async fn error_payload(response: Response) -> SafeErrorPayload {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body reads")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("safe error payload decodes")
    }

    async fn response_json<T: serde::de::DeserializeOwned>(response: Response) -> T {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body reads")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("response payload decodes")
    }

    async fn complete_handshake(router: &Router) {
        let hello = hello(1, 1);
        let response = router
            .clone()
            .oneshot(json_request("/internal/v1/handshake", Some(TOKEN), &hello))
            .await
            .expect("router responds");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn authenticated_handshake_returns_real_core_information() {
        let state = test_state();
        let instance_id = state.instance_id;
        let hello = hello(1, 1);
        let response = build_router(state)
            .oneshot(json_request("/internal/v1/handshake", Some(TOKEN), &hello))
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body reads")
            .to_bytes();
        let core: CoreHello = serde_json::from_slice(&bytes).expect("core hello decodes");
        assert_eq!(core.instance_id, instance_id);
        assert_eq!(core.selected_protocol, PROTOCOL_VERSION);
        assert_eq!(core.correlation_id, hello.correlation_id);
        assert_eq!(core.application.name, "GixGiz");
        assert!(
            core.supported_capabilities
                .contains(&TransportCapability::CapabilityRecommendation)
        );
    }

    #[tokio::test]
    async fn authenticated_recommendation_uses_supplied_profile_without_scanning() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let request = recommendation_request();
        let response = router
            .oneshot(json_request(
                "/internal/v1/recommendations",
                Some(TOKEN),
                &request,
            ))
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::OK);
        let recommendation: RecommendationResponse = response_json(response).await;
        assert_eq!(recommendation.correlation_id, request.correlation_id);
        assert_eq!(recommendation.request_id, request.request_id);
        assert!(recommendation.report.recommended_plan.is_some());
    }

    #[tokio::test]
    async fn recommendation_rejects_unsupported_profile_with_safe_payload() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let mut request = recommendation_request();
        request.machine_profile.schema_version = 999;
        let response = router
            .oneshot(json_request(
                "/internal/v1/recommendations",
                Some(TOKEN),
                &request,
            ))
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let error = error_payload(response).await;
        assert_eq!(error.category, ErrorCategory::IncompatibleVersion);
        assert_eq!(error.code, "capability.machine_profile_incompatible");
        assert_eq!(error.correlation_id, request.correlation_id);
    }

    #[tokio::test]
    async fn recommendation_rejects_unauthenticated_input_before_parsing() {
        let ids = BoundaryIds {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let request = Request::builder()
            .method(Method::POST)
            .uri("/internal/v1/recommendations")
            .header(header::CONTENT_TYPE, "application/json")
            .header(CORRELATION_HEADER, ids.correlation_id.to_string())
            .header(REQUEST_HEADER, ids.request_id.to_string())
            .body(Body::from("not-json"))
            .expect("request builds");
        let response = build_router(test_state())
            .oneshot(request)
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let error = error_payload(response).await;
        assert_eq!(error.code, "transport.authentication_required");
    }

    #[tokio::test]
    async fn recommendation_rejects_invalid_authenticated_payload_safely() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let ids = BoundaryIds {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let request = Request::builder()
            .method(Method::POST)
            .uri("/internal/v1/recommendations")
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(CORRELATION_HEADER, ids.correlation_id.to_string())
            .header(REQUEST_HEADER, ids.request_id.to_string())
            .body(Body::from("not-json"))
            .expect("request builds");
        let response = router.oneshot(request).await.expect("router responds");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error = error_payload(response).await;
        assert_eq!(error.code, "transport.invalid_json_body");
        assert_eq!(error.correlation_id, ids.correlation_id);
    }

    #[tokio::test]
    async fn authenticated_handshake_includes_real_persistence_health() {
        let temporary = tempfile::tempdir().expect("temporary directory is available");
        let context = OperationContext::generated();
        let mut platform = PlatformCore::with_persistence_root(temporary.path().join("data-root"));
        let status = platform
            .start(&context)
            .expect("core reports persistence health");
        let hello = hello(1, 1);
        let response = build_router(state_with_status(status))
            .oneshot(json_request("/internal/v1/handshake", Some(TOKEN), &hello))
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::OK);
        let core: CoreHello = response_json(response).await;
        let persistence = core
            .readiness
            .services
            .iter()
            .find(|service| service.service_id == "persistence")
            .expect("persistence health crosses the handshake");
        assert_eq!(persistence.status, ServiceHealthStatus::Healthy);
        assert_eq!(persistence.requirement, ServiceRequirement::Mandatory);
    }

    #[tokio::test]
    async fn missing_or_invalid_token_is_rejected_before_body_parsing() {
        for token in [
            None,
            Some("cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"),
        ] {
            let ids = BoundaryIds {
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            };
            let mut builder = Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/handshake")
                .header(header::CONTENT_TYPE, "application/json")
                .header(CORRELATION_HEADER, ids.correlation_id.to_string())
                .header(REQUEST_HEADER, ids.request_id.to_string());
            if let Some(token) = token {
                builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
            }
            let request = builder
                .body(Body::from("not-json"))
                .expect("request builds");
            let response = build_router(test_state())
                .oneshot(request)
                .await
                .expect("router responds");

            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            let error = error_payload(response).await;
            assert_eq!(error.code, "transport.authentication_required");
            assert_eq!(error.correlation_id, ids.correlation_id);
            assert_eq!(error.request_id, ids.request_id);
        }
    }

    #[tokio::test]
    async fn incompatible_protocol_fails_closed_with_safe_ids() {
        let hello = hello(2, 3);
        let response = build_router(test_state())
            .oneshot(json_request("/internal/v1/handshake", Some(TOKEN), &hello))
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let error = error_payload(response).await;
        assert_eq!(error.category, ErrorCategory::IncompatibleVersion);
        assert_eq!(error.code, "transport.protocol_incompatible");
        assert_eq!(error.correlation_id, hello.correlation_id);
    }

    #[tokio::test]
    async fn listener_uses_dynamic_ipv4_loopback_port() {
        let state = test_state();
        let host = SidecarHost::bind(
            state.token,
            state.instance_id,
            state.status,
            test_hardware_scanner(),
        )
        .await
        .expect("loopback listener binds");

        assert_eq!(host.local_addr().ip(), std::net::Ipv4Addr::LOCALHOST);
        assert_ne!(host.local_addr().port(), 0);
    }

    #[tokio::test]
    async fn authenticated_sse_stream_delivers_ordered_cancelled_terminal_event() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let correlation_id = CorrelationId::new();
        let start_request = TestOperationStartRequest {
            correlation_id,
            request_id: RequestId::new(),
        };
        let start_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/test-operations",
                Some(TOKEN),
                &start_request,
            ))
            .await
            .expect("start request responds");
        let started: TestOperationStartResponse = response_json(start_response).await;
        let cancel_request = CancelOperationRequest {
            correlation_id,
            request_id: RequestId::new(),
        };
        let cancel_response = router
            .clone()
            .oneshot(json_request(
                &format!(
                    "/internal/v1/test-operations/{}/cancel",
                    started.operation_id
                ),
                Some(TOKEN),
                &cancel_request,
            ))
            .await
            .expect("cancel request responds");
        let cancelled: CancelOperationResponse = response_json(cancel_response).await;
        assert!(cancelled.accepted);

        let event_request_id = RequestId::new();
        let event_request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/test-operations/{}/events",
                started.operation_id
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, correlation_id.to_string())
            .header(REQUEST_HEADER, event_request_id.to_string())
            .body(Body::empty())
            .expect("event request builds");
        let event_response = router
            .oneshot(event_request)
            .await
            .expect("event request responds");
        assert_eq!(event_response.status(), StatusCode::OK);
        let body = event_response
            .into_body()
            .collect()
            .await
            .expect("event stream completes")
            .to_bytes();
        let text = String::from_utf8(body.to_vec()).expect("SSE is UTF-8");
        let payloads = text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|data| {
                serde_json::from_str::<gixgiz_contracts::TestOperationEvent>(data)
                    .expect("event payload decodes")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            payloads
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(
            payloads
                .iter()
                .all(|event| event.correlation_id == correlation_id)
        );
        assert_eq!(
            payloads.last().and_then(|event| event.terminal_state),
            Some(gixgiz_contracts::TestOperationTerminalState::Cancelled)
        );
    }

    #[tokio::test]
    async fn authenticated_hardware_route_streams_a_safe_typed_failure() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let correlation_id = CorrelationId::new();
        let start_request = HardwareScanStartRequest {
            correlation_id,
            request_id: RequestId::new(),
        };
        let start_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/hardware-scans",
                Some(TOKEN),
                &start_request,
            ))
            .await
            .expect("start request responds");
        assert_eq!(start_response.status(), StatusCode::OK);
        let started: HardwareScanStartResponse = response_json(start_response).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        let request_id = RequestId::new();
        let event_request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/hardware-scans/{}/events",
                started.operation_id
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, correlation_id.to_string())
            .header(REQUEST_HEADER, request_id.to_string())
            .body(Body::empty())
            .expect("event request builds");
        let event_response = router
            .oneshot(event_request)
            .await
            .expect("event request responds");
        assert_eq!(event_response.status(), StatusCode::OK);
        let body = event_response
            .into_body()
            .collect()
            .await
            .expect("event stream completes")
            .to_bytes();
        let text = String::from_utf8(body.to_vec()).expect("SSE is UTF-8");
        let events = text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|data| {
                serde_json::from_str::<HardwareScanEvent>(data)
                    .expect("hardware event payload decodes")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let terminal = events.last().expect("terminal event exists");
        assert_eq!(
            terminal.terminal_state,
            Some(gixgiz_contracts::HardwareScanTerminalState::Failed)
        );
        assert_eq!(
            terminal.error.as_ref().map(|error| error.code.as_str()),
            Some("hardware.provider_unavailable")
        );
        assert!(terminal.profile.is_none());
    }

    #[tokio::test]
    async fn shutdown_signal_stops_bound_server_within_deadline() {
        let state = test_state();
        let host = SidecarHost::bind(
            state.token,
            state.instance_id,
            state.status,
            test_hardware_scanner(),
        )
        .await
        .expect("loopback listener binds");
        let shutdown = host.shutdown_handle();
        let server = tokio::spawn(host.run());

        shutdown.request();
        let result = tokio::time::timeout(Duration::from_secs(1), server)
            .await
            .expect("server stops within the deadline")
            .expect("server task joins");

        assert!(result.is_ok());
    }
}
