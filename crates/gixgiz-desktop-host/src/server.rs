use std::{convert::Infallible, net::SocketAddr, str::FromStr, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::Body,
    extract::{
        DefaultBodyLimit, Extension, Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response, Sse, sse::Event, sse::KeepAlive},
    routing::{get, post},
};
use gixgiz_contracts::{
    CancelGenerationRequest, CancelGenerationResponse, CancelOperationRequest,
    CancelOperationResponse, ChatGenerationEvent, ClientHello, ConversationId, CoreHello,
    CorrelationId, CreateConversationRequest, CreateConversationResponse,
    DeleteConversationRequest, DeleteConversationResponse, ErrorCategory, GenerationId,
    GetConversationRequest, GetConversationResponse, HardwareScanEvent, HardwareScanStartRequest,
    HardwareScanStartResponse, HealthRequest, HealthResponse, InstanceId, ListConversationsRequest,
    ListConversationsResponse, OperationId, PROTOCOL_VERSION, PlatformStatus,
    RecommendationRequest, RecommendationResponse, RecoveryAction, RecoveryGuidance,
    RenameConversationRequest, RenameConversationResponse, RequestId, RuntimeConsentRequest,
    RuntimeConsentResponse, RuntimeModelInventoryRequest, RuntimeModelInventoryResponse,
    RuntimeOperationEvent, RuntimeOperationStartRequest, RuntimeOperationStartResponse,
    RuntimeStatusRequest, RuntimeStatusResponse, SafeErrorPayload, SendMessageRequest,
    SendMessageResponse, SetupApprovalRequest, SetupApprovalResponse, SetupJobCancelRequest,
    SetupJobCancelResponse, SetupJobEvent, SetupJobEventsRequest, SetupJobId,
    SetupJobRecoveryRequest, SetupJobRecoveryResponse, SetupJobRetryRequest, SetupJobRetryResponse,
    SetupJobStartRequest, SetupJobStartResponse, SetupJobState, SetupJobStatusRequest,
    SetupJobStatusResponse, SetupPlanRequest, SetupPlanResponse, ShutdownRequest, ShutdownResponse,
    TestOperationStartRequest, TestOperationStartResponse, TransportCapability,
};
use gixgiz_core::{
    CapabilityEngine, ChatService, CoreError, HardwareScanner, OperationContext, RuntimeService,
    SetupService,
};
use gixgiz_runtime::{RuntimeError, RuntimeOperationContext};
use serde::Deserialize;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    HostError,
    bootstrap::BearerToken,
    hardware_scans::HardwareScanRegistry,
    operations::{OperationError, OperationRegistry},
    runtime_operations::RuntimeOperationRegistry,
};

const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_CONCURRENT_REQUESTS: usize = 16;
const MAX_CONCURRENT_EVENT_STREAMS: usize = 16;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const SETUP_EVENT_LIMIT: u32 = 64;
const SETUP_EVENT_CHANNEL_CAPACITY: usize = 16;
const SETUP_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const SETUP_EVENT_KEEP_ALIVE: Duration = Duration::from_secs(15);
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
    runtime: RuntimeService,
    setup: Option<SetupService>,
    chat: Option<ChatService>,
    runtime_operations: RuntimeOperationRegistry,
    request_slots: Arc<Semaphore>,
    event_stream_slots: Arc<Semaphore>,
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
        runtime: RuntimeService,
        setup: Option<SetupService>,
        chat: Option<ChatService>,
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
            chat,
            handshaken: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            operations: OperationRegistry::default(),
            hardware_scans: HardwareScanRegistry::new(hardware_scanner),
            capability_engine: CapabilityEngine::v0_1(),
            runtime_operations: RuntimeOperationRegistry::new(runtime.clone()),
            runtime,
            setup,
            request_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            event_stream_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_EVENT_STREAMS)),
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
        .route("/internal/v1/runtime/status", post(runtime_status))
        .route("/internal/v1/runtime/consent", post(runtime_consent))
        .route("/internal/v1/runtime/models", post(runtime_models))
        .route(
            "/internal/v1/runtime/operations",
            post(start_runtime_operation),
        )
        .route(
            "/internal/v1/runtime/operations/{operation_id}/events",
            get(runtime_operation_events),
        )
        .route(
            "/internal/v1/runtime/operations/{operation_id}/cancel",
            post(cancel_runtime_operation),
        )
        .route("/internal/v1/setup/plan", post(create_setup_plan))
        .route("/internal/v1/setup/jobs", post(start_setup_job))
        .route("/internal/v1/setup/jobs/recovery", post(recover_setup_job))
        .route(
            "/internal/v1/setup/jobs/{job_id}/approve",
            post(decide_setup_approval),
        )
        .route(
            "/internal/v1/setup/jobs/{job_id}/status",
            post(setup_job_status),
        )
        .route(
            "/internal/v1/setup/jobs/{job_id}/events",
            get(setup_job_events),
        )
        .route(
            "/internal/v1/setup/jobs/{job_id}/cancel",
            post(cancel_setup_job),
        )
        .route(
            "/internal/v1/setup/jobs/{job_id}/retry",
            post(retry_setup_job),
        )
        .route("/internal/v1/chat/conversations", post(create_conversation))
        .route(
            "/internal/v1/chat/conversations/list",
            post(list_conversations),
        )
        .route(
            "/internal/v1/chat/conversations/{conversation_id}",
            post(get_conversation),
        )
        .route(
            "/internal/v1/chat/conversations/{conversation_id}/rename",
            post(rename_conversation),
        )
        .route(
            "/internal/v1/chat/conversations/{conversation_id}/delete",
            post(delete_conversation),
        )
        .route(
            "/internal/v1/chat/conversations/{conversation_id}/messages",
            post(send_chat_message),
        )
        .route(
            "/internal/v1/chat/generations/{generation_id}/events",
            get(chat_generation_events),
        )
        .route(
            "/internal/v1/chat/generations/{generation_id}/cancel",
            post(cancel_chat_generation),
        )
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

    if !has_valid_boundary_headers(request.headers()) {
        return ApiFailure::new(
            StatusCode::BAD_REQUEST,
            ErrorCategory::InvalidInput,
            "transport.identifiers_required",
            "Valid request identifiers are required in the transport headers.",
            RecoveryAction::Retry,
            "Retry with valid correlation and request identifiers.",
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
        supported_capabilities: supported_capabilities(state.setup.is_some(), state.chat.is_some()),
        runtime_provider_id: Some(state.runtime.provider_id()),
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
    let stream_permit = acquire_event_stream(&state, ids)?;
    let operation_id =
        OperationId::from_str(&operation_id).map_err(|_| invalid_operation_id(ids))?;
    let mut subscription = state
        .operations
        .subscribe(operation_id)
        .map_err(|error| operation_failure(error, ids))?;
    let (sender, receiver) = mpsc::channel(16);

    tokio::spawn(async move {
        let _stream_permit = stream_permit;
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
    let stream_permit = acquire_event_stream(&state, ids)?;
    let operation_id =
        OperationId::from_str(&operation_id).map_err(|_| invalid_operation_id(ids))?;
    let mut subscription = state
        .hardware_scans
        .subscribe(operation_id, ids.correlation_id)
        .map_err(|error| operation_failure(error, ids))?;
    let (sender, receiver) = mpsc::channel(8);

    tokio::spawn(async move {
        let _stream_permit = stream_permit;
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

async fn runtime_status(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<RuntimeStatusRequest>, JsonRejection>,
) -> Result<Json<RuntimeStatusResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let context =
        RuntimeOperationContext::new(request.correlation_id, request.request_id, REQUEST_TIMEOUT);
    let report = state
        .runtime
        .status(&request.provider_id, context)
        .await
        .map_err(|error| runtime_failure(error, ids))?;
    Ok(Json(RuntimeStatusResponse {
        report,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn runtime_consent(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<RuntimeConsentRequest>, JsonRejection>,
) -> Result<Json<RuntimeConsentResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let context =
        RuntimeOperationContext::new(request.correlation_id, request.request_id, REQUEST_TIMEOUT);
    let report = state
        .runtime
        .set_reuse_consent(&request.provider_id, request.decision, context)
        .await
        .map_err(|error| runtime_failure(error, ids))?;
    Ok(Json(RuntimeConsentResponse {
        report,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn runtime_models(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<RuntimeModelInventoryRequest>, JsonRejection>,
) -> Result<Json<RuntimeModelInventoryResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let context =
        RuntimeOperationContext::new(request.correlation_id, request.request_id, REQUEST_TIMEOUT);
    let inventory = state
        .runtime
        .list_models(&request.provider_id, request.limit, context)
        .await
        .map_err(|error| runtime_failure(error, ids))?;
    Ok(Json(RuntimeModelInventoryResponse {
        inventory,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn start_runtime_operation(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<RuntimeOperationStartRequest>, JsonRejection>,
) -> Result<Json<RuntimeOperationStartResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let operation_id = state
        .runtime_operations
        .start(
            request.provider_id,
            request.kind,
            request.correlation_id,
            request.request_id,
        )
        .map_err(|error| runtime_operation_failure(error, ids))?;
    Ok(Json(RuntimeOperationStartResponse {
        operation_id,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn runtime_operation_events(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(operation_id): Path<String>,
) -> Result<Sse<ReceiverStream<Result<Event, Infallible>>>, ApiFailure> {
    let stream_permit = acquire_event_stream(&state, ids)?;
    let operation_id =
        OperationId::from_str(&operation_id).map_err(|_| invalid_operation_id(ids))?;
    let mut subscription = state
        .runtime_operations
        .subscribe(operation_id, ids.correlation_id)
        .map_err(|error| runtime_operation_failure(error, ids))?;
    let (sender, receiver) = mpsc::channel(8);

    tokio::spawn(async move {
        let _stream_permit = stream_permit;
        for event in subscription.replay {
            let terminal = event.terminal_state.is_some();
            if send_runtime_event(&sender, event).await.is_err() || terminal {
                return;
            }
        }
        if subscription.terminal {
            return;
        }
        while let Ok(event) = subscription.receiver.recv().await {
            let terminal = event.terminal_state.is_some();
            if send_runtime_event(&sender, event).await.is_err() || terminal {
                return;
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(receiver)))
}

async fn cancel_runtime_operation(
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
        .runtime_operations
        .cancel(operation_id, request.correlation_id)
        .map_err(|error| runtime_operation_failure(error, ids))?;
    Ok(Json(CancelOperationResponse {
        operation_id,
        accepted,
        correlation_id: request.correlation_id,
        request_id: request.request_id,
    }))
}

async fn create_setup_plan(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<SetupPlanRequest>, JsonRejection>,
) -> Result<Json<SetupPlanResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let service = setup_service(&state, ids)?;
    service
        .create_plan(request)
        .await
        .map(Json)
        .map_err(|error| setup_failure(error, ids))
}

async fn recover_setup_job(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<SetupJobRecoveryRequest>, JsonRejection>,
) -> Result<Json<SetupJobRecoveryResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let service = setup_service(&state, ids)?;
    service
        .recover_job(request)
        .await
        .map(Json)
        .map_err(|error| setup_failure(error, ids))
}

async fn decide_setup_approval(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(job_id): Path<String>,
    payload: Result<Json<SetupApprovalRequest>, JsonRejection>,
) -> Result<Json<SetupApprovalResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    ensure_setup_job_path(&job_id, request.job_id, ids)?;
    let service = setup_service(&state, ids)?;
    service
        .decide_approval(request)
        .await
        .map(Json)
        .map_err(|error| setup_failure(error, ids))
}

async fn start_setup_job(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<SetupJobStartRequest>, JsonRejection>,
) -> Result<Json<SetupJobStartResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    let service = setup_service(&state, ids)?;
    service
        .start_job(request)
        .await
        .map(Json)
        .map_err(|error| setup_failure(error, ids))
}

async fn setup_job_status(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(job_id): Path<String>,
    payload: Result<Json<SetupJobStatusRequest>, JsonRejection>,
) -> Result<Json<SetupJobStatusResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    ensure_setup_job_path(&job_id, request.job_id, ids)?;
    let service = setup_service(&state, ids)?;
    service
        .status(request)
        .await
        .map(Json)
        .map_err(|error| setup_failure(error, ids))
}

#[derive(Clone, Copy, Deserialize)]
struct SetupEventsQuery {
    #[serde(default)]
    after_sequence: u64,
    #[serde(default = "default_setup_event_limit")]
    limit: u32,
}

const fn default_setup_event_limit() -> u32 {
    SETUP_EVENT_LIMIT
}

async fn setup_job_events(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(job_id): Path<String>,
    query: Result<Query<SetupEventsQuery>, QueryRejection>,
) -> Result<Response, ApiFailure> {
    let job_id = parse_setup_job_id(&job_id, ids)?;
    let query = query.map_err(|_| invalid_setup_events_query(ids))?.0;
    if query.limit == 0 || query.limit > SETUP_EVENT_LIMIT || query.after_sequence > i64::MAX as u64
    {
        return Err(invalid_setup_events_query(ids));
    }
    let stream_permit = acquire_event_stream(&state, ids)?;
    let service = setup_service(&state, ids)?.clone();
    let first_page = service
        .events_after(SetupJobEventsRequest {
            job_id,
            after_sequence: query.after_sequence,
            limit: query.limit,
            correlation_id: ids.correlation_id,
            request_id: ids.request_id,
        })
        .await
        .map_err(|error| setup_failure(error, ids))?;
    validate_setup_event_page(&first_page.events, job_id, query.after_sequence, ids)?;
    let (sender, receiver) = mpsc::channel(SETUP_EVENT_CHANNEL_CAPACITY);

    tokio::spawn(async move {
        let _stream_permit = stream_permit;
        let mut cursor = query.after_sequence;
        let mut sent = 0_u32;
        let mut next_page = Some(first_page);
        loop {
            if sender.is_closed() {
                return;
            }
            let remaining = query.limit.saturating_sub(sent);
            if remaining == 0 {
                return;
            }
            let page = match next_page.take() {
                Some(page) => page,
                None => match service
                    .events_after(SetupJobEventsRequest {
                        job_id,
                        after_sequence: cursor,
                        limit: remaining,
                        correlation_id: ids.correlation_id,
                        request_id: ids.request_id,
                    })
                    .await
                {
                    Ok(page) => page,
                    Err(_) => return,
                },
            };
            let page_was_empty = page.events.is_empty();
            for event in page.events {
                if event.job_id != job_id
                    || event.job.job_id != job_id
                    || !is_next_setup_sequence(cursor, event.sequence)
                {
                    return;
                }
                cursor = event.sequence;
                sent = sent.saturating_add(1);
                let terminal = event.terminal_state.is_some();
                if send_setup_event(&sender, event).await.is_err()
                    || terminal
                    || sent >= query.limit
                {
                    return;
                }
            }
            if page_was_empty {
                let status = service
                    .status(SetupJobStatusRequest {
                        job_id,
                        correlation_id: ids.correlation_id,
                        request_id: ids.request_id,
                    })
                    .await;
                match status {
                    Ok(response) if setup_state_is_terminal(response.job.state) => return,
                    Ok(_) => tokio::time::sleep(SETUP_EVENT_POLL_INTERVAL).await,
                    Err(_) => return,
                }
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(receiver))
        .keep_alive(
            KeepAlive::new()
                .interval(SETUP_EVENT_KEEP_ALIVE)
                .text("keep-alive"),
        )
        .into_response())
}

async fn cancel_setup_job(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(job_id): Path<String>,
    payload: Result<Json<SetupJobCancelRequest>, JsonRejection>,
) -> Result<Json<SetupJobCancelResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    ensure_setup_job_path(&job_id, request.job_id, ids)?;
    let service = setup_service(&state, ids)?;
    service
        .cancel_job(request)
        .await
        .map(Json)
        .map_err(|error| setup_failure(error, ids))
}

async fn retry_setup_job(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(job_id): Path<String>,
    payload: Result<Json<SetupJobRetryRequest>, JsonRejection>,
) -> Result<Json<SetupJobRetryResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    ensure_setup_job_path(&job_id, request.job_id, ids)?;
    let service = setup_service(&state, ids)?;
    service
        .retry_job(request)
        .await
        .map(Json)
        .map_err(|error| setup_failure(error, ids))
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

async fn send_runtime_event(
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    event: RuntimeOperationEvent,
) -> Result<(), ()> {
    let sequence = event.sequence.to_string();
    let data = serde_json::to_string(&event).map_err(|_| ())?;
    sender
        .send(Ok(Event::default()
            .event("runtime_operation")
            .id(sequence)
            .data(data)))
        .await
        .map_err(|_| ())
}

async fn send_setup_event(
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    event: SetupJobEvent,
) -> Result<(), ()> {
    let sequence = event.sequence.to_string();
    let data = serde_json::to_string(&event).map_err(|_| ())?;
    sender
        .send(Ok(Event::default()
            .event("setup_job")
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

fn supported_capabilities(setup_available: bool, chat_available: bool) -> Vec<TransportCapability> {
    let mut capabilities = vec![
        TransportCapability::Health,
        TransportCapability::TestOperationEvents,
        TransportCapability::HardwareScan,
        TransportCapability::CapabilityRecommendation,
        TransportCapability::RuntimeStatus,
        TransportCapability::RuntimeConsent,
        TransportCapability::RuntimeLifecycle,
        TransportCapability::RuntimeModelInventory,
        TransportCapability::Cancellation,
        TransportCapability::Shutdown,
    ];
    if setup_available {
        capabilities.push(TransportCapability::SetupWorkflow);
    }
    if chat_available {
        capabilities.push(TransportCapability::LocalChat);
    }
    capabilities
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

fn has_valid_boundary_headers(headers: &HeaderMap) -> bool {
    headers
        .get(CORRELATION_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| CorrelationId::from_str(value).is_ok())
        && headers
            .get(REQUEST_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| RequestId::from_str(value).is_ok())
}

fn acquire_event_stream(
    state: &AppState,
    ids: BoundaryIds,
) -> Result<OwnedSemaphorePermit, ApiFailure> {
    state
        .event_stream_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            ApiFailure::new(
                StatusCode::TOO_MANY_REQUESTS,
                ErrorCategory::ResourceExhausted,
                "transport.concurrent_event_stream_limit",
                "The local core is handling the maximum number of event streams.",
                RecoveryAction::Retry,
                "Close an existing event stream, then retry.",
                ids,
            )
        })
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

fn parse_setup_job_id(value: &str, ids: BoundaryIds) -> Result<SetupJobId, ApiFailure> {
    SetupJobId::from_str(value).map_err(|_| {
        ApiFailure::new(
            StatusCode::BAD_REQUEST,
            ErrorCategory::InvalidInput,
            "setup.job_id_invalid",
            "The model setup job identifier was invalid.",
            RecoveryAction::NoAction,
            "Return to the model setup screen and refresh its persisted state.",
            ids,
        )
    })
}

fn ensure_setup_job_path(
    value: &str,
    expected: SetupJobId,
    ids: BoundaryIds,
) -> Result<(), ApiFailure> {
    if parse_setup_job_id(value, ids)? == expected {
        return Ok(());
    }
    Err(ApiFailure::new(
        StatusCode::BAD_REQUEST,
        ErrorCategory::InvalidInput,
        "setup.job_id_mismatch",
        "The model setup job identifier did not match the request body.",
        RecoveryAction::Retry,
        "Refresh the persisted setup state before retrying.",
        ids,
    ))
}

fn invalid_setup_events_query(ids: BoundaryIds) -> ApiFailure {
    ApiFailure::new(
        StatusCode::BAD_REQUEST,
        ErrorCategory::InvalidInput,
        "setup.events_query_invalid",
        "The model setup event cursor or limit was invalid.",
        RecoveryAction::Retry,
        "Retry with a non-negative cursor and an event limit from 1 through 64.",
        ids,
    )
}

fn validate_setup_event_page(
    events: &[SetupJobEvent],
    job_id: SetupJobId,
    after_sequence: u64,
    ids: BoundaryIds,
) -> Result<(), ApiFailure> {
    let mut cursor = after_sequence;
    for event in events {
        if event.job_id != job_id
            || event.job.job_id != job_id
            || !is_next_setup_sequence(cursor, event.sequence)
        {
            return Err(ApiFailure::new(
                StatusCode::CONFLICT,
                ErrorCategory::Conflict,
                "setup.event_cursor_stale",
                "The persisted setup event cursor is no longer current.",
                RecoveryAction::Retry,
                "Refresh the authoritative setup status before reconnecting to events.",
                ids,
            ));
        }
        cursor = event.sequence;
    }
    Ok(())
}

fn is_next_setup_sequence(previous: u64, current: u64) -> bool {
    previous.checked_add(1) == Some(current)
}

fn setup_service(state: &AppState, ids: BoundaryIds) -> Result<&SetupService, ApiFailure> {
    state.setup.as_ref().ok_or_else(|| {
        ApiFailure::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCategory::Unavailable,
            "setup.service_unavailable",
            "Persistent model setup is unavailable.",
            RecoveryAction::Restart,
            "Restart GixGiz, then refresh the model setup state.",
            ids,
        )
    })
}

const fn setup_state_is_terminal(state: SetupJobState) -> bool {
    matches!(
        state,
        SetupJobState::AttentionRequired
            | SetupJobState::Ready
            | SetupJobState::Failed
            | SetupJobState::Cancelled
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

fn setup_failure(error: CoreError, ids: BoundaryIds) -> ApiFailure {
    let context = OperationContext::new(ids.correlation_id, ids.request_id);
    let payload = error.to_safe_payload(&context);
    let status = match payload.category {
        ErrorCategory::InvalidInput => StatusCode::BAD_REQUEST,
        ErrorCategory::PermissionDenied => StatusCode::FORBIDDEN,
        ErrorCategory::NotSupported => StatusCode::NOT_IMPLEMENTED,
        ErrorCategory::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCategory::Conflict | ErrorCategory::IncompatibleVersion => StatusCode::CONFLICT,
        ErrorCategory::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
        ErrorCategory::Cancelled => StatusCode::CONFLICT,
        ErrorCategory::TimedOut => StatusCode::GATEWAY_TIMEOUT,
        ErrorCategory::IntegrityFailure | ErrorCategory::Degraded | ErrorCategory::Internal => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        ErrorCategory::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiFailure {
        status,
        payload: Box::new(payload),
    }
}

fn runtime_operation_failure(error: OperationError, ids: BoundaryIds) -> ApiFailure {
    match error {
        OperationError::Busy => ApiFailure::new(
            StatusCode::CONFLICT,
            ErrorCategory::Conflict,
            "runtime.operation_busy",
            "A runtime lifecycle operation is already in progress.",
            RecoveryAction::Retry,
            "Wait for the current operation or cancel it before retrying.",
            ids,
        ),
        other => operation_failure(other, ids),
    }
}

fn runtime_failure(error: RuntimeError, ids: BoundaryIds) -> ApiFailure {
    let context = RuntimeOperationContext::new(ids.correlation_id, ids.request_id, REQUEST_TIMEOUT);
    let payload = error.to_safe_payload(&context);
    let status = match payload.category {
        ErrorCategory::InvalidInput => StatusCode::BAD_REQUEST,
        ErrorCategory::PermissionDenied => StatusCode::FORBIDDEN,
        ErrorCategory::NotSupported => StatusCode::NOT_IMPLEMENTED,
        ErrorCategory::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCategory::Conflict | ErrorCategory::IncompatibleVersion => StatusCode::CONFLICT,
        ErrorCategory::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
        ErrorCategory::Cancelled => StatusCode::CONFLICT,
        ErrorCategory::TimedOut => StatusCode::GATEWAY_TIMEOUT,
        ErrorCategory::IntegrityFailure | ErrorCategory::Degraded | ErrorCategory::Internal => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        ErrorCategory::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
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

const CHAT_EVENT_CHANNEL_CAPACITY: usize = 32;

#[derive(Debug, Deserialize)]
struct ChatEventsQuery {
    #[serde(default)]
    after_sequence: u64,
}

fn chat_service(state: &AppState, ids: BoundaryIds) -> Result<&ChatService, ApiFailure> {
    state.chat.as_ref().ok_or_else(|| {
        ApiFailure::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCategory::Unavailable,
            "chat.service_unavailable",
            "Local chat is unavailable.",
            RecoveryAction::Restart,
            "Restart GixGiz, then open the chat workspace again.",
            ids,
        )
    })
}

fn chat_failure(error: CoreError, ids: BoundaryIds) -> ApiFailure {
    let context = OperationContext::new(ids.correlation_id, ids.request_id);
    let payload = error.to_safe_payload(&context);
    let status = match payload.category {
        ErrorCategory::InvalidInput => StatusCode::BAD_REQUEST,
        ErrorCategory::PermissionDenied => StatusCode::FORBIDDEN,
        ErrorCategory::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCategory::Conflict | ErrorCategory::IncompatibleVersion | ErrorCategory::Cancelled => {
            StatusCode::CONFLICT
        }
        ErrorCategory::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
        ErrorCategory::TimedOut => StatusCode::GATEWAY_TIMEOUT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiFailure {
        status,
        payload: Box::new(payload),
    }
}

fn parse_conversation_path(value: &str, ids: BoundaryIds) -> Result<ConversationId, ApiFailure> {
    ConversationId::from_str(value).map_err(|_| {
        ApiFailure::new(
            StatusCode::BAD_REQUEST,
            ErrorCategory::InvalidInput,
            "chat.invalid_conversation_id",
            "The conversation identifier was invalid.",
            RecoveryAction::NoAction,
            "Reload the conversation list, then try again.",
            ids,
        )
    })
}

fn parse_generation_path(value: &str, ids: BoundaryIds) -> Result<GenerationId, ApiFailure> {
    GenerationId::from_str(value).map_err(|_| {
        ApiFailure::new(
            StatusCode::BAD_REQUEST,
            ErrorCategory::InvalidInput,
            "chat.invalid_generation_id",
            "The reply identifier was invalid.",
            RecoveryAction::NoAction,
            "Reload the conversation, then try again.",
            ids,
        )
    })
}

fn chat_identifier_mismatch(ids: BoundaryIds) -> ApiFailure {
    ApiFailure::new(
        StatusCode::BAD_REQUEST,
        ErrorCategory::InvalidInput,
        "chat.identifier_mismatch",
        "The request path and body referred to different records.",
        RecoveryAction::Retry,
        "Retry with matching identifiers.",
        ids,
    )
}

fn invalid_chat_events_query(ids: BoundaryIds) -> ApiFailure {
    ApiFailure::new(
        StatusCode::BAD_REQUEST,
        ErrorCategory::InvalidInput,
        "chat.invalid_events_query",
        "The reply stream cursor was invalid.",
        RecoveryAction::Retry,
        "Reload the conversation, then observe the reply again.",
        ids,
    )
}

async fn create_conversation(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<CreateConversationRequest>, JsonRejection>,
) -> Result<Json<CreateConversationResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    chat_service(&state, ids)?
        .create_conversation(request)
        .await
        .map(Json)
        .map_err(|error| chat_failure(error, ids))
}

async fn list_conversations(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    payload: Result<Json<ListConversationsRequest>, JsonRejection>,
) -> Result<Json<ListConversationsResponse>, ApiFailure> {
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    chat_service(&state, ids)?
        .list_conversations(request)
        .await
        .map(Json)
        .map_err(|error| chat_failure(error, ids))
}

async fn get_conversation(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(conversation_id): Path<String>,
    payload: Result<Json<GetConversationRequest>, JsonRejection>,
) -> Result<Json<GetConversationResponse>, ApiFailure> {
    let conversation_id = parse_conversation_path(&conversation_id, ids)?;
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    if request.conversation_id != conversation_id {
        return Err(chat_identifier_mismatch(ids));
    }
    chat_service(&state, ids)?
        .get_conversation(request)
        .await
        .map(Json)
        .map_err(|error| chat_failure(error, ids))
}

async fn rename_conversation(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(conversation_id): Path<String>,
    payload: Result<Json<RenameConversationRequest>, JsonRejection>,
) -> Result<Json<RenameConversationResponse>, ApiFailure> {
    let conversation_id = parse_conversation_path(&conversation_id, ids)?;
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    if request.conversation_id != conversation_id {
        return Err(chat_identifier_mismatch(ids));
    }
    chat_service(&state, ids)?
        .rename_conversation(request)
        .await
        .map(Json)
        .map_err(|error| chat_failure(error, ids))
}

async fn delete_conversation(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(conversation_id): Path<String>,
    payload: Result<Json<DeleteConversationRequest>, JsonRejection>,
) -> Result<Json<DeleteConversationResponse>, ApiFailure> {
    let conversation_id = parse_conversation_path(&conversation_id, ids)?;
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    if request.conversation_id != conversation_id {
        return Err(chat_identifier_mismatch(ids));
    }
    chat_service(&state, ids)?
        .delete_conversation(request)
        .await
        .map(Json)
        .map_err(|error| chat_failure(error, ids))
}

async fn send_chat_message(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(conversation_id): Path<String>,
    payload: Result<Json<SendMessageRequest>, JsonRejection>,
) -> Result<Json<SendMessageResponse>, ApiFailure> {
    let conversation_id = parse_conversation_path(&conversation_id, ids)?;
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    if request.conversation_id != conversation_id {
        return Err(chat_identifier_mismatch(ids));
    }
    // Returns once the generation is admitted; output arrives on the event stream.
    chat_service(&state, ids)?
        .send_message(request)
        .await
        .map(Json)
        .map_err(|error| chat_failure(error, ids))
}

async fn cancel_chat_generation(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(generation_id): Path<String>,
    payload: Result<Json<CancelGenerationRequest>, JsonRejection>,
) -> Result<Json<CancelGenerationResponse>, ApiFailure> {
    let generation_id = parse_generation_path(&generation_id, ids)?;
    let request = parse_json(payload, ids)?.0;
    ensure_ids(request.correlation_id, request.request_id, ids)?;
    if request.generation_id != generation_id {
        return Err(chat_identifier_mismatch(ids));
    }
    chat_service(&state, ids)?
        .cancel_generation(request)
        .await
        .map(Json)
        .map_err(|error| chat_failure(error, ids))
}

async fn chat_generation_events(
    State(state): State<AppState>,
    Extension(ids): Extension<BoundaryIds>,
    Path(generation_id): Path<String>,
    query: Result<Query<ChatEventsQuery>, QueryRejection>,
) -> Result<Response, ApiFailure> {
    let generation_id = parse_generation_path(&generation_id, ids)?;
    let query = query.map_err(|_| invalid_chat_events_query(ids))?.0;
    if query.after_sequence > i64::MAX as u64 {
        return Err(invalid_chat_events_query(ids));
    }
    let stream_permit = acquire_event_stream(&state, ids)?;
    let subscription = chat_service(&state, ids)?
        .observe(generation_id, query.after_sequence)
        .map_err(|error| chat_failure(error, ids))?;
    let (sender, receiver) = mpsc::channel(CHAT_EVENT_CHANNEL_CAPACITY);

    tokio::spawn(async move {
        let _stream_permit = stream_permit;
        let mut live = subscription.receiver;
        for event in subscription.replay {
            let terminal = event.terminal_state.is_some();
            if send_chat_event(&sender, event).await.is_err() || terminal {
                return;
            }
        }
        if subscription.terminal {
            return;
        }
        while let Ok(event) = live.recv().await {
            let terminal = event.terminal_state.is_some();
            if send_chat_event(&sender, event).await.is_err() || terminal {
                return;
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(receiver)).into_response())
}

async fn send_chat_event(
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    event: ChatGenerationEvent,
) -> Result<(), ()> {
    let sequence = event.sequence.to_string();
    let data = serde_json::to_string(&event).map_err(|_| ())?;
    sender
        .send(Ok(Event::default()
            .event("chat_generation")
            .id(sequence)
            .data(data)))
        .await
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use std::{
        future,
        path::Path as FilePath,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use axum::http::Request;
    use gixgiz_contracts::{
        AccelerationEvidence, AccelerationKind, ArchitectureEvidence, CandidateModelId,
        CpuEvidence, EvidenceConfidence, EvidenceMetadata, EvidenceSource, GpuCollectionEvidence,
        MachineArchitecture, MachineProfile, MachineProfileCompleteness, ModelAcquisitionPhase,
        ModelAcquisitionProgress, ModelIntegrityState, ModelProviderArtifact,
        OperatingSystemEvidence, PhysicalMemoryEvidence, PreferencePriority,
        ProviderRegistrationResult, ProviderRegistrationState, RuntimeCapabilityAvailability,
        RuntimeCapabilityDescriptor, RuntimeCapabilityKind, RuntimeConsentDecision,
        RuntimeConsentState, RuntimeDisplayName, RuntimeEndpointSafety, RuntimeModelInventory,
        RuntimeOwnership, RuntimeProviderId, RuntimeProviderModelId, RuntimeState,
        ServiceHealthStatus, ServiceRequirement, SetupApprovalDecision, SetupDestinationCategory,
        SetupJobEventKind, SetupJobRecoveryResponse, SetupJobRetryResponse, SetupJobSnapshot,
        SetupJobStartResponse, SetupJobStatusResponse, SetupPlanResponse, SetupStage,
        StorageEvidence, StorageLocation, StorageMediaEvidence, StorageMediaKind, StringEvidence,
        U32Evidence, U64Evidence, UserPreferenceProfile, WorkloadTier,
    };
    use gixgiz_core::{
        CollectedHardwareEvidence, CoreError, HardwareProvider, OperationContext, PlatformCore,
    };
    use gixgiz_runtime::{
        ChatDeltaSender, ModelProgressSender, RuntimeCancellationSemantics, RuntimeChatProvider,
        RuntimeChatRequest, RuntimeDetector, RuntimeError, RuntimeFuture, RuntimeGenerationResult,
        RuntimeLifecycle, RuntimeModelAcquisitionPlan, RuntimeModelAcquisitionResult,
        RuntimeModelAcquisitionStatus, RuntimeModelInspection, RuntimeModelInventoryProvider,
        RuntimeModelSetupProvider, RuntimeObservation, RuntimeOperationContext, RuntimeProvider,
        RuntimeReadinessInferenceResult, RuntimeStorageAvailability, RuntimeStoragePreflight,
        testing::FakeRuntimeProvider,
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;

    const TOKEN: &str = "abababababababababababababababababababababababababababababababab";

    struct UnavailableProvider;

    struct CancellableRuntimeProvider {
        provider_id: RuntimeProviderId,
        detect_calls: AtomicUsize,
        first_detect_started: Arc<tokio::sync::Notify>,
    }

    struct SetupRuntimeProvider {
        provider_id: RuntimeProviderId,
        artifact: ModelProviderArtifact,
        acquisition_calls: AtomicUsize,
        detect_calls: AtomicUsize,
        detect_failures: AtomicUsize,
        first_acquisition_started: tokio::sync::Notify,
    }

    impl SetupRuntimeProvider {
        fn new() -> Self {
            let provider_id = RuntimeProviderId::new("gixgiz.runtime.setup-test.v1");
            Self {
                artifact: ModelProviderArtifact {
                    canonical_model_id: CandidateModelId::new("qwen2.5.1.5b-instruct"),
                    provider_id: provider_id.clone(),
                    provider_model_id: RuntimeProviderModelId::new("fixture:1.5b"),
                    source_summary: "Repository-owned test mapping".to_owned(),
                },
                provider_id,
                acquisition_calls: AtomicUsize::new(0),
                detect_calls: AtomicUsize::new(0),
                detect_failures: AtomicUsize::new(0),
                first_acquisition_started: tokio::sync::Notify::new(),
            }
        }

        fn observation(&self) -> RuntimeObservation {
            RuntimeObservation {
                provider_id: self.provider_id.clone(),
                display_name: RuntimeDisplayName::new("Setup test runtime"),
                state: RuntimeState::Ready,
                endpoint_safety: RuntimeEndpointSafety::LoopbackVerified,
                version: None,
                capabilities: vec![RuntimeCapabilityDescriptor {
                    kind: RuntimeCapabilityKind::ModelInventory,
                    availability: RuntimeCapabilityAvailability::Available,
                    reason: None,
                }],
                reasons: Vec::new(),
                warnings: Vec::new(),
            }
        }
    }

    impl RuntimeDetector for SetupRuntimeProvider {
        fn detect(
            &self,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            Box::pin(async move {
                self.detect_calls.fetch_add(1, Ordering::AcqRel);
                if let Err(error) = context.check() {
                    self.detect_failures.fetch_add(1, Ordering::AcqRel);
                    return Err(error);
                }
                Ok(self.observation())
            })
        }
    }

    impl RuntimeLifecycle for SetupRuntimeProvider {
        fn execute(
            &self,
            _kind: gixgiz_contracts::RuntimeOperationKind,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            Box::pin(async move {
                context.check()?;
                Ok(self.observation())
            })
        }
    }

    impl RuntimeModelInventoryProvider for SetupRuntimeProvider {
        fn list_models(
            &self,
            _limit: u16,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInventory> {
            Box::pin(async move {
                context.check()?;
                Ok(RuntimeModelInventory {
                    schema_version: gixgiz_contracts::RUNTIME_REPORT_SCHEMA_VERSION,
                    provider_id: self.provider_id.clone(),
                    models: Vec::new(),
                    truncated: false,
                    collected_at_unix_ms: 1,
                })
            })
        }
    }

    impl RuntimeModelSetupProvider for SetupRuntimeProvider {
        fn prepare_model_acquisition(
            &self,
            model_id: CandidateModelId,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionPlan> {
            Box::pin(async move {
                context.check()?;
                if model_id != self.artifact.canonical_model_id {
                    return Err(RuntimeError::ModelNotMapped);
                }
                Ok(RuntimeModelAcquisitionPlan {
                    artifact: self.artifact.clone(),
                    destination: SetupDestinationCategory::ProviderManaged,
                    destination_display: "Provider-managed test storage".to_owned(),
                    cancellation: RuntimeCancellationSemantics::ConnectionAbortMayRetainEffects,
                })
            })
        }

        fn preflight_model_storage(
            &self,
            _plan: RuntimeModelAcquisitionPlan,
            required_bytes: u64,
            safety_margin_bytes: u64,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeStoragePreflight> {
            Box::pin(async move {
                context.check()?;
                Ok(RuntimeStoragePreflight {
                    destination: SetupDestinationCategory::ProviderManaged,
                    destination_display: "Provider-managed test storage".to_owned(),
                    availability: RuntimeStorageAvailability::Available,
                    required_bytes,
                    safety_margin_bytes,
                    available_bytes: Some(required_bytes + safety_margin_bytes + 1),
                    checked_at_unix_ms: 10,
                })
            })
        }

        fn acquire_model(
            &self,
            _plan: RuntimeModelAcquisitionPlan,
            progress: ModelProgressSender,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionResult> {
            Box::pin(async move {
                if self.acquisition_calls.fetch_add(1, Ordering::AcqRel) == 0 {
                    self.first_acquisition_started.notify_one();
                    return context
                        .run(future::pending::<
                            Result<RuntimeModelAcquisitionResult, RuntimeError>,
                        >())
                        .await;
                }
                context.check()?;
                for update in [
                    ModelAcquisitionProgress {
                        phase: ModelAcquisitionPhase::Transferring,
                        completed_bytes: Some(512 * 1024 * 1024),
                        total_bytes: Some(2 * 1024 * 1024 * 1024),
                        progress_basis_points: Some(2_500),
                    },
                    ModelAcquisitionProgress {
                        phase: ModelAcquisitionPhase::Transferring,
                        completed_bytes: Some(1024 * 1024 * 1024),
                        total_bytes: Some(2 * 1024 * 1024 * 1024),
                        progress_basis_points: Some(5_000),
                    },
                    ModelAcquisitionProgress {
                        phase: ModelAcquisitionPhase::Completed,
                        completed_bytes: Some(2 * 1024 * 1024 * 1024),
                        total_bytes: Some(2 * 1024 * 1024 * 1024),
                        progress_basis_points: Some(10_000),
                    },
                ] {
                    let _ = progress.try_send(update);
                }
                Ok(RuntimeModelAcquisitionResult {
                    artifact: self.artifact.clone(),
                    status: RuntimeModelAcquisitionStatus::Acquired,
                    measured_size_bytes: Some(2 * 1024 * 1024 * 1024),
                    integrity: ModelIntegrityState::ProviderReported,
                    completed_at_unix_ms: 20,
                })
            })
        }

        fn inspect_model(
            &self,
            artifact: ModelProviderArtifact,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInspection> {
            Box::pin(async move {
                context.check()?;
                let acquired = self.acquisition_calls.load(Ordering::Acquire) >= 2;
                Ok(RuntimeModelInspection {
                    artifact,
                    available: acquired,
                    registration: ProviderRegistrationResult {
                        state: if acquired {
                            ProviderRegistrationState::Registered
                        } else {
                            ProviderRegistrationState::NotRegistered
                        },
                        measured_size_bytes: acquired.then_some(2 * 1024 * 1024 * 1024),
                        verified_at_unix_ms: 30,
                    },
                    integrity: if acquired {
                        ModelIntegrityState::ProviderReported
                    } else {
                        ModelIntegrityState::Unavailable
                    },
                })
            })
        }

        fn run_readiness_inference(
            &self,
            _artifact: ModelProviderArtifact,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeReadinessInferenceResult> {
            Box::pin(async move {
                context.check()?;
                Ok(RuntimeReadinessInferenceResult {
                    ready: true,
                    completed_at_unix_ms: 40,
                })
            })
        }
    }

    impl RuntimeChatProvider for SetupRuntimeProvider {
        fn generate(
            &self,
            _request: RuntimeChatRequest,
            _deltas: ChatDeltaSender,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeGenerationResult> {
            // This double covers setup behavior only; chat is exercised elsewhere.
            Box::pin(async move {
                context.check()?;
                Err(RuntimeError::Unsupported)
            })
        }
    }

    impl RuntimeProvider for SetupRuntimeProvider {
        fn provider_id(&self) -> &RuntimeProviderId {
            &self.provider_id
        }
    }

    impl CancellableRuntimeProvider {
        fn new(first_detect_started: Arc<tokio::sync::Notify>) -> Self {
            Self {
                provider_id: RuntimeProviderId::new("gixgiz.runtime.cancel-test.v1"),
                detect_calls: AtomicUsize::new(0),
                first_detect_started,
            }
        }

        fn observation(&self) -> RuntimeObservation {
            RuntimeObservation {
                provider_id: self.provider_id.clone(),
                display_name: RuntimeDisplayName::new("Cancellation test runtime"),
                state: RuntimeState::Ready,
                endpoint_safety: RuntimeEndpointSafety::LoopbackVerified,
                version: None,
                capabilities: Vec::new(),
                reasons: Vec::new(),
                warnings: Vec::new(),
            }
        }
    }

    impl RuntimeDetector for CancellableRuntimeProvider {
        fn detect(
            &self,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            Box::pin(async move {
                if self.detect_calls.fetch_add(1, Ordering::AcqRel) == 0 {
                    self.first_detect_started.notify_one();
                    return context
                        .run(std::future::pending::<
                            Result<RuntimeObservation, RuntimeError>,
                        >())
                        .await;
                }
                context.check()?;
                Ok(self.observation())
            })
        }
    }

    impl RuntimeLifecycle for CancellableRuntimeProvider {
        fn execute(
            &self,
            _kind: gixgiz_contracts::RuntimeOperationKind,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            Box::pin(async move {
                context.check()?;
                Ok(self.observation())
            })
        }
    }

    impl RuntimeModelInventoryProvider for CancellableRuntimeProvider {
        fn list_models(
            &self,
            _limit: u16,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInventory> {
            Box::pin(async move {
                context.check()?;
                Ok(RuntimeModelInventory {
                    schema_version: gixgiz_contracts::RUNTIME_REPORT_SCHEMA_VERSION,
                    provider_id: self.provider_id.clone(),
                    models: Vec::new(),
                    truncated: false,
                    collected_at_unix_ms: 1,
                })
            })
        }
    }

    impl RuntimeModelSetupProvider for CancellableRuntimeProvider {
        fn prepare_model_acquisition(
            &self,
            _model_id: CandidateModelId,
            _context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionPlan> {
            Box::pin(async { Err(RuntimeError::Unsupported) })
        }

        fn preflight_model_storage(
            &self,
            _plan: RuntimeModelAcquisitionPlan,
            _required_bytes: u64,
            _safety_margin_bytes: u64,
            _context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeStoragePreflight> {
            Box::pin(async { Err(RuntimeError::Unsupported) })
        }

        fn acquire_model(
            &self,
            _plan: RuntimeModelAcquisitionPlan,
            _progress: ModelProgressSender,
            _context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionResult> {
            Box::pin(async { Err(RuntimeError::Unsupported) })
        }

        fn inspect_model(
            &self,
            _artifact: ModelProviderArtifact,
            _context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInspection> {
            Box::pin(async { Err(RuntimeError::Unsupported) })
        }

        fn run_readiness_inference(
            &self,
            _artifact: ModelProviderArtifact,
            _context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeReadinessInferenceResult> {
            Box::pin(async { Err(RuntimeError::Unsupported) })
        }
    }

    impl RuntimeChatProvider for CancellableRuntimeProvider {
        fn generate(
            &self,
            _request: RuntimeChatRequest,
            _deltas: ChatDeltaSender,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeGenerationResult> {
            // This double covers setup behavior only; chat is exercised elsewhere.
            Box::pin(async move {
                context.check()?;
                Err(RuntimeError::Unsupported)
            })
        }
    }

    impl RuntimeProvider for CancellableRuntimeProvider {
        fn provider_id(&self) -> &RuntimeProviderId {
            &self.provider_id
        }
    }

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

    fn test_runtime_service() -> RuntimeService {
        let provider_id = RuntimeProviderId::new("gixgiz.runtime.test.v1");
        let observation = RuntimeObservation {
            provider_id: provider_id.clone(),
            display_name: RuntimeDisplayName::new("Test runtime"),
            state: RuntimeState::Ready,
            endpoint_safety: RuntimeEndpointSafety::LoopbackVerified,
            version: None,
            capabilities: vec![
                RuntimeCapabilityDescriptor {
                    kind: RuntimeCapabilityKind::Detection,
                    availability: RuntimeCapabilityAvailability::Available,
                    reason: None,
                },
                RuntimeCapabilityDescriptor {
                    kind: RuntimeCapabilityKind::ModelInventory,
                    availability: RuntimeCapabilityAvailability::Available,
                    reason: None,
                },
            ],
            reasons: Vec::new(),
            warnings: Vec::new(),
        };
        let inventory = RuntimeModelInventory {
            schema_version: gixgiz_contracts::RUNTIME_REPORT_SCHEMA_VERSION,
            provider_id: provider_id.clone(),
            models: Vec::new(),
            truncated: false,
            collected_at_unix_ms: 1,
        };
        RuntimeService::in_memory(Arc::new(FakeRuntimeProvider::new(
            provider_id,
            Ok(observation.clone()),
            Ok(observation),
            Ok(inventory),
        )))
    }

    fn state_with_status_and_runtime(status: PlatformStatus, runtime: RuntimeService) -> AppState {
        let (shutdown_sender, _) = watch::channel(false);
        AppState {
            token: BearerToken::parse(TOKEN.to_owned()).expect("fixed token is valid"),
            instance_id: InstanceId::new(),
            status,
            chat: None,
            handshaken: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            operations: OperationRegistry::default(),
            hardware_scans: HardwareScanRegistry::new(test_hardware_scanner()),
            capability_engine: CapabilityEngine::v0_1(),
            runtime_operations: RuntimeOperationRegistry::new(runtime.clone()),
            runtime,
            setup: None,
            request_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            event_stream_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_EVENT_STREAMS)),
            shutdown: ShutdownHandle {
                sender: shutdown_sender,
            },
        }
    }

    fn state_with_status(status: PlatformStatus) -> AppState {
        state_with_status_and_runtime(status, test_runtime_service())
    }

    fn test_state() -> AppState {
        let context = OperationContext::generated();
        let mut core = PlatformCore::new(Vec::new());
        let status = core
            .start(&context)
            .expect("core starts with fake-free status");
        state_with_status(status)
    }

    fn test_setup_state(root: &FilePath, provider: Arc<dyn RuntimeProvider>) -> AppState {
        let context = OperationContext::generated();
        let mut core = PlatformCore::with_persistence_root(root);
        let runtime = core.runtime_service(provider.clone());
        let setup = core
            .setup_service(provider)
            .expect("temporary persistence composes setup");
        let status = core.start(&context).expect("core starts with persistence");
        let mut state = state_with_status_and_runtime(status, runtime);
        state.setup = Some(setup);
        state
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

    async fn wait_for_setup_state(
        router: &Router,
        job_id: SetupJobId,
        expected: SetupJobState,
    ) -> SetupJobSnapshot {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let request = SetupJobStatusRequest {
                    job_id,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                };
                let response = router
                    .clone()
                    .oneshot(json_request(
                        &format!("/internal/v1/setup/jobs/{job_id}/status"),
                        Some(TOKEN),
                        &request,
                    ))
                    .await
                    .expect("setup status request responds");
                assert_eq!(response.status(), StatusCode::OK);
                let response: SetupJobStatusResponse = response_json(response).await;
                if response.job.state == expected {
                    return response.job;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("setup reaches expected durable state")
    }

    /// Builds a host with a real chat service over an isolated database.
    ///
    /// The temporary directory must outlive the state, so it is returned too.
    fn chat_state() -> (tempfile::TempDir, AppState) {
        let temporary = tempfile::tempdir().expect("temporary directory is available");
        let context = OperationContext::generated();
        let mut core = PlatformCore::with_persistence_root(temporary.path().join("data-root"));
        let provider: Arc<dyn RuntimeProvider> = Arc::new(FakeRuntimeProvider::new(
            RuntimeProviderId::new("gixgiz.runtime.test.v1"),
            Ok(chat_observation()),
            Ok(chat_observation()),
            Ok(RuntimeModelInventory {
                schema_version: gixgiz_contracts::RUNTIME_REPORT_SCHEMA_VERSION,
                provider_id: RuntimeProviderId::new("gixgiz.runtime.test.v1"),
                models: Vec::new(),
                truncated: false,
                collected_at_unix_ms: 1,
            }),
        ));
        let chat = core.chat_service(provider);
        let status = core.start(&context).expect("core starts with persistence");
        let mut state = state_with_status(status);
        state.chat = chat;
        (temporary, state)
    }

    fn chat_observation() -> RuntimeObservation {
        RuntimeObservation {
            provider_id: RuntimeProviderId::new("gixgiz.runtime.test.v1"),
            display_name: RuntimeDisplayName::new("Test runtime"),
            state: RuntimeState::Ready,
            endpoint_safety: RuntimeEndpointSafety::LoopbackVerified,
            version: None,
            capabilities: Vec::new(),
            reasons: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn conversation_path(conversation_id: ConversationId, action: &str) -> String {
        if action.is_empty() {
            format!("/internal/v1/chat/conversations/{conversation_id}")
        } else {
            format!("/internal/v1/chat/conversations/{conversation_id}/{action}")
        }
    }

    async fn create_conversation_via_router(
        router: &Router,
    ) -> gixgiz_contracts::ConversationSnapshot {
        let request = CreateConversationRequest {
            title: None,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/chat/conversations",
                Some(TOKEN),
                &request,
            ))
            .await
            .expect("router responds");
        assert_eq!(response.status(), StatusCode::OK);
        response_json::<CreateConversationResponse>(response)
            .await
            .conversation
    }

    #[tokio::test]
    async fn local_chat_capability_is_advertised_only_when_the_service_exists() {
        let (_temporary, state) = chat_state();
        let with_chat = build_router(state);
        let without_chat = build_router(test_state());

        let enabled: CoreHello = response_json(
            with_chat
                .oneshot(json_request(
                    "/internal/v1/handshake",
                    Some(TOKEN),
                    &hello(1, 1),
                ))
                .await
                .expect("router responds"),
        )
        .await;
        let disabled: CoreHello = response_json(
            without_chat
                .oneshot(json_request(
                    "/internal/v1/handshake",
                    Some(TOKEN),
                    &hello(1, 1),
                ))
                .await
                .expect("router responds"),
        )
        .await;

        assert!(
            enabled
                .supported_capabilities
                .contains(&TransportCapability::LocalChat)
        );
        assert!(
            !disabled
                .supported_capabilities
                .contains(&TransportCapability::LocalChat)
        );
    }

    #[tokio::test]
    async fn authenticated_conversation_crud_round_trips_through_the_boundary() {
        let (_temporary, state) = chat_state();
        let router = build_router(state);
        complete_handshake(&router).await;

        let created = create_conversation_via_router(&router).await;
        let conversation_id = created.conversation_id;

        let list_request = ListConversationsRequest {
            limit: 50,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let listed: ListConversationsResponse = response_json(
            router
                .clone()
                .oneshot(json_request(
                    "/internal/v1/chat/conversations/list",
                    Some(TOKEN),
                    &list_request,
                ))
                .await
                .expect("router responds"),
        )
        .await;

        let rename_request = RenameConversationRequest {
            conversation_id,
            title: "Renamed thread".to_owned(),
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let renamed: RenameConversationResponse = response_json(
            router
                .clone()
                .oneshot(json_request(
                    &conversation_path(conversation_id, "rename"),
                    Some(TOKEN),
                    &rename_request,
                ))
                .await
                .expect("router responds"),
        )
        .await;

        let get_request = GetConversationRequest {
            conversation_id,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let fetched: GetConversationResponse = response_json(
            router
                .clone()
                .oneshot(json_request(
                    &conversation_path(conversation_id, ""),
                    Some(TOKEN),
                    &get_request,
                ))
                .await
                .expect("router responds"),
        )
        .await;

        let delete_request = DeleteConversationRequest {
            conversation_id,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let deleted: DeleteConversationResponse = response_json(
            router
                .clone()
                .oneshot(json_request(
                    &conversation_path(conversation_id, "delete"),
                    Some(TOKEN),
                    &delete_request,
                ))
                .await
                .expect("router responds"),
        )
        .await;

        let missing = router
            .oneshot(json_request(
                &conversation_path(conversation_id, ""),
                Some(TOKEN),
                &get_request,
            ))
            .await
            .expect("router responds");

        assert_eq!(listed.conversations.len(), 1);
        assert!(!listed.truncated);
        assert_eq!(renamed.conversation.title, "Renamed thread");
        assert_eq!(fetched.conversation.title, "Renamed thread");
        assert!(deleted.deleted);
        assert_eq!(missing.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn chat_routes_reject_unauthenticated_and_unhandshaken_callers() {
        let (_temporary, state) = chat_state();
        let router = build_router(state);
        let request = ListConversationsRequest {
            limit: 50,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };

        let unauthenticated = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/chat/conversations/list",
                None,
                &request,
            ))
            .await
            .expect("router responds");
        let unhandshaken = router
            .oneshot(json_request(
                "/internal/v1/chat/conversations/list",
                Some(TOKEN),
                &request,
            ))
            .await
            .expect("router responds");

        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_payload(unauthenticated).await.code,
            "transport.authentication_required"
        );
        assert_eq!(unhandshaken.status(), StatusCode::PRECONDITION_REQUIRED);
        assert_eq!(
            error_payload(unhandshaken).await.code,
            "transport.handshake_required"
        );
    }

    #[tokio::test]
    async fn chat_path_and_body_identifiers_must_match() {
        let (_temporary, state) = chat_state();
        let router = build_router(state);
        complete_handshake(&router).await;
        let created = create_conversation_via_router(&router).await;

        let mismatched = GetConversationRequest {
            conversation_id: ConversationId::new(),
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let response = router
            .oneshot(json_request(
                &conversation_path(created.conversation_id, ""),
                Some(TOKEN),
                &mismatched,
            ))
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_payload(response).await.code,
            "chat.identifier_mismatch"
        );
    }

    #[tokio::test]
    async fn chat_rejects_an_invalid_body_and_an_invalid_conversation_path() {
        let (_temporary, state) = chat_state();
        let router = build_router(state);
        complete_handshake(&router).await;
        let ids = BoundaryIds {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };

        let invalid_body = Request::builder()
            .method(Method::POST)
            .uri("/internal/v1/chat/conversations")
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(CORRELATION_HEADER, ids.correlation_id.to_string())
            .header(REQUEST_HEADER, ids.request_id.to_string())
            .body(Body::from("not-json"))
            .expect("request builds");
        let body_response = router
            .clone()
            .oneshot(invalid_body)
            .await
            .expect("router responds");

        let path_request = GetConversationRequest {
            conversation_id: ConversationId::new(),
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let path_response = router
            .oneshot(json_request(
                "/internal/v1/chat/conversations/not-a-uuid",
                Some(TOKEN),
                &path_request,
            ))
            .await
            .expect("router responds");

        assert_eq!(body_response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_payload(body_response).await.code,
            "transport.invalid_json_body"
        );
        assert_eq!(path_response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_payload(path_response).await.code,
            "chat.invalid_conversation_id"
        );
    }

    #[tokio::test]
    async fn chat_routes_fail_safely_without_a_durable_service() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let request = ListConversationsRequest {
            limit: 50,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };

        let response = router
            .oneshot(json_request(
                "/internal/v1/chat/conversations/list",
                Some(TOKEN),
                &request,
            ))
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            error_payload(response).await.code,
            "chat.service_unavailable"
        );
    }

    #[tokio::test]
    async fn sending_without_recorded_reuse_consent_maps_to_a_safe_actionable_payload() {
        let (_temporary, state) = chat_state();
        let router = build_router(state);
        complete_handshake(&router).await;
        let created = create_conversation_via_router(&router).await;

        let send = SendMessageRequest {
            conversation_id: created.conversation_id,
            content: "hello".to_owned(),
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let response = router
            .oneshot(json_request(
                &conversation_path(created.conversation_id, "messages"),
                Some(TOKEN),
                &send,
            ))
            .await
            .expect("router responds");

        // Chat inherits the runtime reuse-consent gate; it fires before any
        // model lookup, so the boundary reports permission rather than absence.
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let payload = error_payload(response).await;
        assert_eq!(payload.category, ErrorCategory::PermissionDenied);
        assert_eq!(payload.code, "chat.runtime_consent_required");
        assert_eq!(payload.correlation_id, send.correlation_id);
        // The provider is never named in a user-facing chat failure.
        assert!(!payload.message.to_ascii_lowercase().contains("ollama"));
    }

    #[tokio::test]
    async fn unknown_generations_fail_closed_for_observation_and_cancellation() {
        let (_temporary, state) = chat_state();
        let router = build_router(state);
        complete_handshake(&router).await;
        let generation_id = GenerationId::new();

        let cancel = CancelGenerationRequest {
            generation_id,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let cancelled: CancelGenerationResponse = response_json(
            router
                .clone()
                .oneshot(json_request(
                    &format!("/internal/v1/chat/generations/{generation_id}/cancel"),
                    Some(TOKEN),
                    &cancel,
                ))
                .await
                .expect("router responds"),
        )
        .await;

        let ids = BoundaryIds {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let events = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/chat/generations/{generation_id}/events?after_sequence=0"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, ids.correlation_id.to_string())
            .header(REQUEST_HEADER, ids.request_id.to_string())
            .body(Body::empty())
            .expect("request builds");
        let events_response = router.oneshot(events).await.expect("router responds");

        assert!(!cancelled.accepted);
        assert_eq!(events_response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            error_payload(events_response).await.code,
            "chat.generation_not_found"
        );
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
        assert_eq!(
            core.runtime_provider_id,
            Some(RuntimeProviderId::new("gixgiz.runtime.test.v1"))
        );
        assert!(
            !core
                .supported_capabilities
                .contains(&TransportCapability::SetupWorkflow)
        );
    }

    #[tokio::test]
    async fn authenticated_setup_routes_cancel_retry_recover_and_replay_durable_state() {
        let temporary = tempfile::tempdir().expect("temporary directory is available");
        let provider = Arc::new(SetupRuntimeProvider::new());
        let provider_service: Arc<dyn RuntimeProvider> = provider.clone();
        let router = build_router(test_setup_state(temporary.path(), provider_service));
        let handshake = hello(PROTOCOL_VERSION, PROTOCOL_VERSION);
        let handshake_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/handshake",
                Some(TOKEN),
                &handshake,
            ))
            .await
            .expect("setup handshake responds");
        assert_eq!(handshake_response.status(), StatusCode::OK);
        let hello: CoreHello = response_json(handshake_response).await;
        assert!(
            hello
                .supported_capabilities
                .contains(&TransportCapability::SetupWorkflow)
        );

        let consent_request = RuntimeConsentRequest {
            provider_id: provider.provider_id.clone(),
            decision: RuntimeConsentDecision::ApproveReuse,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let consent_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/runtime/consent",
                Some(TOKEN),
                &consent_request,
            ))
            .await
            .expect("runtime consent responds");
        assert_eq!(consent_response.status(), StatusCode::OK);
        let consent: RuntimeConsentResponse = response_json(consent_response).await;
        assert_eq!(
            consent.report.reuse_consent,
            RuntimeConsentState::ReuseApproved
        );

        let recommendation_request = recommendation_request();
        let recommendation_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/recommendations",
                Some(TOKEN),
                &recommendation_request,
            ))
            .await
            .expect("recommendation responds");
        assert_eq!(recommendation_response.status(), StatusCode::OK);
        let recommendation: RecommendationResponse = response_json(recommendation_response).await;
        let selected = recommendation
            .report
            .recommended_plan
            .expect("fixture has a recommended model");
        assert_eq!(
            selected.model.catalogue_id,
            provider.artifact.canonical_model_id
        );

        let plan_request = SetupPlanRequest {
            recommendation: selected,
            provider_id: provider.provider_id.clone(),
            destination: SetupDestinationCategory::ProviderManaged,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let plan_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/setup/plan",
                Some(TOKEN),
                &plan_request,
            ))
            .await
            .expect("setup plan responds");
        assert_eq!(plan_response.status(), StatusCode::OK);
        let planned: SetupPlanResponse = response_json(plan_response).await;
        assert_eq!(planned.job.state, SetupJobState::AwaitingApproval);
        let job_id = planned.job.job_id;
        let plan_revision = planned.job.plan.revision;

        let approval_request = SetupApprovalRequest {
            job_id,
            plan_revision,
            decision: SetupApprovalDecision::Approve,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let approval_response = router
            .clone()
            .oneshot(json_request(
                &format!("/internal/v1/setup/jobs/{job_id}/approve"),
                Some(TOKEN),
                &approval_request,
            ))
            .await
            .expect("setup approval responds");
        assert_eq!(approval_response.status(), StatusCode::OK);
        let approved: SetupApprovalResponse = response_json(approval_response).await;
        assert_eq!(approved.job.state, SetupJobState::Approved);
        assert_eq!(approved.approval.decision, SetupApprovalDecision::Approve);
        assert!(approved.approval.external_runtime_effect);

        let runtime_status_request = RuntimeStatusRequest {
            provider_id: provider.provider_id.clone(),
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let runtime_status_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/runtime/status",
                Some(TOKEN),
                &runtime_status_request,
            ))
            .await
            .expect("runtime status responds before setup start");
        assert_eq!(runtime_status_response.status(), StatusCode::OK);
        let runtime_status: RuntimeStatusResponse = response_json(runtime_status_response).await;
        assert_eq!(runtime_status.report.provider_id, provider.provider_id);
        assert_eq!(
            runtime_status.report.display_name.as_str(),
            "Setup test runtime"
        );
        assert_eq!(runtime_status.report.state, RuntimeState::Ready);
        assert_eq!(runtime_status.report.ownership, RuntimeOwnership::External);
        assert_eq!(
            runtime_status.report.reuse_consent,
            RuntimeConsentState::ReuseApproved
        );
        assert_eq!(runtime_status.report.version, None);

        let start_request = SetupJobStartRequest {
            job_id,
            plan_revision,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let start_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/setup/jobs",
                Some(TOKEN),
                &start_request,
            ))
            .await
            .expect("setup start responds");
        assert_eq!(start_response.status(), StatusCode::OK);
        let started: SetupJobStartResponse = response_json(start_response).await;
        assert_eq!(started.job.job_id, job_id);

        if tokio::time::timeout(
            Duration::from_secs(2),
            provider.first_acquisition_started.notified(),
        )
        .await
        .is_err()
        {
            let status_request = SetupJobStatusRequest {
                job_id,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            };
            let status_response = router
                .clone()
                .oneshot(json_request(
                    &format!("/internal/v1/setup/jobs/{job_id}/status"),
                    Some(TOKEN),
                    &status_request,
                ))
                .await
                .expect("diagnostic setup status responds");
            let status: SetupJobStatusResponse = response_json(status_response).await;
            panic!(
                "first fake acquisition did not start after {} detects ({} context failures): {:?}",
                provider.detect_calls.load(Ordering::Acquire),
                provider.detect_failures.load(Ordering::Acquire),
                status.job
            );
        }
        let cancel_request = SetupJobCancelRequest {
            job_id,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let cancel_response = router
            .clone()
            .oneshot(json_request(
                &format!("/internal/v1/setup/jobs/{job_id}/cancel"),
                Some(TOKEN),
                &cancel_request,
            ))
            .await
            .expect("setup cancel responds");
        assert_eq!(cancel_response.status(), StatusCode::OK);
        let cancelled: SetupJobCancelResponse = response_json(cancel_response).await;
        assert!(cancelled.accepted);
        let cancelled_snapshot =
            wait_for_setup_state(&router, job_id, SetupJobState::Cancelled).await;

        let retry_request = SetupJobRetryRequest {
            job_id,
            plan_revision,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let retry_response = router
            .clone()
            .oneshot(json_request(
                &format!("/internal/v1/setup/jobs/{job_id}/retry"),
                Some(TOKEN),
                &retry_request,
            ))
            .await
            .expect("setup retry responds");
        assert_eq!(retry_response.status(), StatusCode::OK);
        let retried: SetupJobRetryResponse = response_json(retry_response).await;
        assert_eq!(retried.job.job_id, job_id);
        let ready = wait_for_setup_state(&router, job_id, SetupJobState::Ready).await;
        assert_eq!(ready.retry_count, 1);

        let recovery_request = SetupJobRecoveryRequest {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let recovery_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/setup/jobs/recovery",
                Some(TOKEN),
                &recovery_request,
            ))
            .await
            .expect("setup recovery responds");
        assert_eq!(recovery_response.status(), StatusCode::OK);
        let recovered: SetupJobRecoveryResponse = response_json(recovery_response).await;
        assert_eq!(
            recovered.job.map(|job| job.state),
            Some(SetupJobState::Ready)
        );

        let event_correlation = CorrelationId::new();
        let event_request_id = RequestId::new();
        let event_request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/setup/jobs/{job_id}/events?after_sequence={}&limit=64",
                cancelled_snapshot.latest_event_sequence
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, event_correlation.to_string())
            .header(REQUEST_HEADER, event_request_id.to_string())
            .body(Body::empty())
            .expect("setup event request builds");
        let event_response = router
            .oneshot(event_request)
            .await
            .expect("setup event request responds");
        assert_eq!(event_response.status(), StatusCode::OK);
        let event_body = tokio::time::timeout(Duration::from_secs(2), async {
            event_response
                .into_body()
                .collect()
                .await
                .expect("setup event body reads")
                .to_bytes()
        })
        .await
        .expect("terminal setup event stream closes");
        let event_text = String::from_utf8(event_body.to_vec()).expect("SSE is UTF-8");
        assert!(event_text.contains("event: setup_job"));
        assert!(event_text.contains(&format!(
            "id: {}",
            cancelled_snapshot.latest_event_sequence + 1
        )));
        assert!(event_text.contains("\"state\":\"ready\""));
        let replayed: Vec<SetupJobEvent> = event_text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|data| serde_json::from_str(data).expect("setup SSE event decodes"))
            .collect();
        let progress_basis_points: Vec<u16> = replayed
            .iter()
            .filter(|event| {
                event.kind == SetupJobEventKind::Progress
                    && event.job.stage == SetupStage::Acquiring
            })
            .filter_map(|event| {
                event
                    .job
                    .progress
                    .as_ref()
                    .and_then(|progress| progress.progress_basis_points)
            })
            .collect();
        assert_eq!(progress_basis_points, vec![2_500, 5_000, 10_000]);
    }

    #[test]
    fn setup_event_sequence_rejects_gaps_duplicates_and_overflow() {
        assert!(is_next_setup_sequence(0, 1));
        assert!(is_next_setup_sequence(41, 42));
        assert!(!is_next_setup_sequence(41, 41));
        assert!(!is_next_setup_sequence(41, 43));
        assert!(!is_next_setup_sequence(u64::MAX, 0));
    }

    #[tokio::test]
    async fn setup_events_reject_invalid_cursor_bounds_before_service_access() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let job_id = SetupJobId::new();
        let ids = BoundaryIds {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/setup/jobs/{job_id}/events?after_sequence=0&limit=65"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, ids.correlation_id.to_string())
            .header(REQUEST_HEADER, ids.request_id.to_string())
            .body(Body::empty())
            .expect("setup event request builds");

        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("setup event request responds");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error = error_payload(response).await;
        assert_eq!(error.code, "setup.events_query_invalid");
        assert_eq!(error.correlation_id, ids.correlation_id);
        assert_eq!(error.request_id, ids.request_id);

        let cursor_ids = BoundaryIds {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let cursor_request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/setup/jobs/{job_id}/events?after_sequence={}&limit=1",
                i64::MAX as u64 + 1
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, cursor_ids.correlation_id.to_string())
            .header(REQUEST_HEADER, cursor_ids.request_id.to_string())
            .body(Body::empty())
            .expect("setup cursor request builds");
        let cursor_response = router
            .oneshot(cursor_request)
            .await
            .expect("setup cursor request responds");
        assert_eq!(cursor_response.status(), StatusCode::BAD_REQUEST);
        let cursor_error = error_payload(cursor_response).await;
        assert_eq!(cursor_error.code, "setup.events_query_invalid");
        assert_eq!(cursor_error.correlation_id, cursor_ids.correlation_id);
        assert_eq!(cursor_error.request_id, cursor_ids.request_id);
    }

    #[tokio::test]
    async fn setup_routes_require_headers_and_fail_safely_without_durable_service() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let job_id = SetupJobId::new();
        let correlation_id = CorrelationId::new();
        let missing_header_request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/setup/jobs/{job_id}/events?after_sequence=0&limit=1"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, correlation_id.to_string())
            .body(Body::empty())
            .expect("setup event request builds");
        let missing_header_response = router
            .clone()
            .oneshot(missing_header_request)
            .await
            .expect("setup event request responds");
        assert_eq!(missing_header_response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_payload(missing_header_response).await.code,
            "transport.identifiers_required"
        );

        let recovery_request = SetupJobRecoveryRequest {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let recovery_response = router
            .oneshot(json_request(
                "/internal/v1/setup/jobs/recovery",
                Some(TOKEN),
                &recovery_request,
            ))
            .await
            .expect("setup recovery responds");
        assert_eq!(recovery_response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let error = error_payload(recovery_response).await;
        assert_eq!(error.code, "setup.service_unavailable");
        assert_eq!(error.category, ErrorCategory::Unavailable);
        assert_eq!(error.correlation_id, recovery_request.correlation_id);
        assert_eq!(error.request_id, recovery_request.request_id);
    }

    #[tokio::test]
    async fn setup_path_and_body_job_identifiers_must_match() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let path_job_id = SetupJobId::new();
        let request = SetupJobStatusRequest {
            job_id: SetupJobId::new(),
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };

        let response = router
            .oneshot(json_request(
                &format!("/internal/v1/setup/jobs/{path_job_id}/status"),
                Some(TOKEN),
                &request,
            ))
            .await
            .expect("setup status request responds");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error_payload(response).await.code, "setup.job_id_mismatch");
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
            test_runtime_service(),
            None,
            None,
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
    async fn authenticated_runtime_status_returns_normalized_external_policy() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let request = RuntimeStatusRequest {
            provider_id: RuntimeProviderId::new("gixgiz.runtime.test.v1"),
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let response = router
            .oneshot(json_request(
                "/internal/v1/runtime/status",
                Some(TOKEN),
                &request,
            ))
            .await
            .expect("runtime status responds");

        assert_eq!(response.status(), StatusCode::OK);
        let status: RuntimeStatusResponse = response_json(response).await;
        assert_eq!(status.report.state, RuntimeState::Ready);
        assert_eq!(
            status.report.ownership,
            gixgiz_contracts::RuntimeOwnership::External
        );
        assert_eq!(status.correlation_id, request.correlation_id);
        assert_eq!(status.request_id, request.request_id);
    }

    #[tokio::test]
    async fn runtime_routes_require_authentication_and_handshake_before_parsing() {
        let request = RuntimeStatusRequest {
            provider_id: RuntimeProviderId::new("gixgiz.runtime.test.v1"),
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let unauthenticated = build_router(test_state())
            .oneshot(json_request("/internal/v1/runtime/status", None, &request))
            .await
            .expect("unauthenticated request responds");
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_payload(unauthenticated).await.code,
            "transport.authentication_required"
        );

        let before_handshake = build_router(test_state())
            .oneshot(json_request(
                "/internal/v1/runtime/status",
                Some(TOKEN),
                &request,
            ))
            .await
            .expect("pre-handshake request responds");
        assert_eq!(before_handshake.status(), StatusCode::PRECONDITION_REQUIRED);
        assert_eq!(
            error_payload(before_handshake).await.code,
            "transport.handshake_required"
        );
    }

    #[tokio::test]
    async fn authenticated_runtime_route_rejects_invalid_payload_safely() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let ids = BoundaryIds {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let request = Request::builder()
            .method(Method::POST)
            .uri("/internal/v1/runtime/status")
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
        assert_eq!(error.request_id, ids.request_id);
    }

    #[tokio::test]
    async fn runtime_event_stream_requires_a_valid_request_id_header() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/runtime/operations/{}/events",
                OperationId::new()
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, CorrelationId::new().to_string())
            .body(Body::empty())
            .expect("event request builds");

        let response = router.oneshot(request).await.expect("router responds");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_payload(response).await.code,
            "transport.identifiers_required"
        );
    }

    #[tokio::test]
    async fn runtime_event_streams_respect_the_global_stream_bound() {
        let state = test_state();
        let slots = state.event_stream_slots.clone();
        let mut permits = Vec::new();
        for _ in 0..MAX_CONCURRENT_EVENT_STREAMS {
            permits.push(
                slots
                    .clone()
                    .try_acquire_owned()
                    .expect("test reserves event stream slot"),
            );
        }
        let router = build_router(state);
        complete_handshake(&router).await;
        let request_id = RequestId::new();
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/runtime/operations/{}/events",
                OperationId::new()
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, CorrelationId::new().to_string())
            .header(REQUEST_HEADER, request_id.to_string())
            .body(Body::empty())
            .expect("event request builds");

        let response = router.oneshot(request).await.expect("router responds");

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            error_payload(response).await.code,
            "transport.concurrent_event_stream_limit"
        );
        drop(permits);
    }

    #[tokio::test]
    async fn runtime_reuse_consent_enables_bounded_inventory_without_adoption() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let models = RuntimeModelInventoryRequest {
            provider_id: RuntimeProviderId::new("gixgiz.runtime.test.v1"),
            limit: 10,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let denied = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/runtime/models",
                Some(TOKEN),
                &models,
            ))
            .await
            .expect("model request responds");
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        assert_eq!(error_payload(denied).await.code, "runtime.consent_required");

        let consent = RuntimeConsentRequest {
            provider_id: models.provider_id.clone(),
            decision: gixgiz_contracts::RuntimeConsentDecision::ApproveReuse,
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let consent_response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/runtime/consent",
                Some(TOKEN),
                &consent,
            ))
            .await
            .expect("consent request responds");
        assert_eq!(consent_response.status(), StatusCode::OK);
        let consent_result: RuntimeConsentResponse = response_json(consent_response).await;
        assert_eq!(
            consent_result.report.ownership,
            gixgiz_contracts::RuntimeOwnership::External
        );
        assert_eq!(
            consent_result.report.reuse_consent,
            gixgiz_contracts::RuntimeConsentState::ReuseApproved
        );
        assert_eq!(
            consent_result.report.management_consent,
            gixgiz_contracts::RuntimeConsentState::NotRequested
        );

        let response = router
            .oneshot(json_request(
                "/internal/v1/runtime/models",
                Some(TOKEN),
                &models,
            ))
            .await
            .expect("approved model request responds");
        assert_eq!(response.status(), StatusCode::OK);
        let inventory: RuntimeModelInventoryResponse = response_json(response).await;
        assert!(inventory.inventory.models.is_empty());
        assert!(!inventory.inventory.truncated);
    }

    #[tokio::test]
    async fn authenticated_runtime_cancel_reaches_provider_and_streams_authoritative_state() {
        let first_detect_started = Arc::new(tokio::sync::Notify::new());
        let provider = Arc::new(CancellableRuntimeProvider::new(
            first_detect_started.clone(),
        ));
        let provider_id = provider.provider_id().clone();
        let base_state = test_state();
        let runtime = RuntimeService::in_memory(provider);
        let router = build_router(state_with_status_and_runtime(base_state.status, runtime));
        complete_handshake(&router).await;
        let correlation_id = CorrelationId::new();
        let start = RuntimeOperationStartRequest {
            provider_id,
            kind: gixgiz_contracts::RuntimeOperationKind::Start,
            correlation_id,
            request_id: RequestId::new(),
        };
        let response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/runtime/operations",
                Some(TOKEN),
                &start,
            ))
            .await
            .expect("runtime operation starts");
        assert_eq!(response.status(), StatusCode::OK);
        let started: RuntimeOperationStartResponse = response_json(response).await;
        tokio::time::timeout(Duration::from_secs(1), first_detect_started.notified())
            .await
            .expect("provider detection starts");

        let cancel = CancelOperationRequest {
            correlation_id,
            request_id: RequestId::new(),
        };
        let response = router
            .clone()
            .oneshot(json_request(
                &format!(
                    "/internal/v1/runtime/operations/{}/cancel",
                    started.operation_id
                ),
                Some(TOKEN),
                &cancel,
            ))
            .await
            .expect("runtime cancellation responds");
        assert_eq!(response.status(), StatusCode::OK);
        let cancellation: CancelOperationResponse = response_json(response).await;
        assert!(cancellation.accepted);

        let events_request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/runtime/operations/{}/events",
                started.operation_id
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, correlation_id.to_string())
            .header(REQUEST_HEADER, RequestId::new().to_string())
            .body(Body::empty())
            .expect("event request builds");
        let response = router
            .oneshot(events_request)
            .await
            .expect("event request responds");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("event stream completes")
            .to_bytes();
        let events = String::from_utf8(body.to_vec())
            .expect("SSE is UTF-8")
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|data| {
                serde_json::from_str::<RuntimeOperationEvent>(data).expect("runtime event decodes")
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
            Some(gixgiz_contracts::RuntimeOperationTerminalState::Cancelled)
        );
        assert!(terminal.error.is_none());
        let report = terminal
            .report
            .as_ref()
            .expect("cancelled operation includes authoritative retained state");
        assert_eq!(report.state, RuntimeState::Ready);
        assert_eq!(
            report.ownership,
            gixgiz_contracts::RuntimeOwnership::External
        );
    }

    #[tokio::test]
    async fn external_runtime_lifecycle_streams_safe_failed_terminal_event() {
        let router = build_router(test_state());
        complete_handshake(&router).await;
        let correlation_id = CorrelationId::new();
        let start = RuntimeOperationStartRequest {
            provider_id: RuntimeProviderId::new("gixgiz.runtime.test.v1"),
            kind: gixgiz_contracts::RuntimeOperationKind::Start,
            correlation_id,
            request_id: RequestId::new(),
        };
        let response = router
            .clone()
            .oneshot(json_request(
                "/internal/v1/runtime/operations",
                Some(TOKEN),
                &start,
            ))
            .await
            .expect("runtime operation starts");
        assert_eq!(response.status(), StatusCode::OK);
        let started: RuntimeOperationStartResponse = response_json(response).await;
        tokio::time::sleep(Duration::from_millis(25)).await;

        let events_request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/internal/v1/runtime/operations/{}/events",
                started.operation_id
            ))
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(CORRELATION_HEADER, correlation_id.to_string())
            .header(REQUEST_HEADER, RequestId::new().to_string())
            .body(Body::empty())
            .expect("event request builds");
        let response = router
            .oneshot(events_request)
            .await
            .expect("event request responds");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("event stream completes")
            .to_bytes();
        let events = String::from_utf8(body.to_vec())
            .expect("SSE is UTF-8")
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|data| {
                serde_json::from_str::<RuntimeOperationEvent>(data).expect("runtime event decodes")
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
            Some(gixgiz_contracts::RuntimeOperationTerminalState::Failed)
        );
        assert_eq!(
            terminal.error.as_ref().map(|error| error.code.as_str()),
            Some("runtime.ownership_conflict")
        );
        let report = terminal
            .report
            .as_ref()
            .expect("failed lifecycle includes authoritative retained state");
        assert_eq!(report.state, RuntimeState::Ready);
        assert_eq!(
            report.ownership,
            gixgiz_contracts::RuntimeOwnership::External
        );
    }

    #[tokio::test]
    async fn shutdown_signal_stops_bound_server_within_deadline() {
        let state = test_state();
        let host = SidecarHost::bind(
            state.token,
            state.instance_id,
            state.status,
            test_hardware_scanner(),
            test_runtime_service(),
            None,
            None,
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
