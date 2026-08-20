use std::{
    env,
    future::Future,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gixgiz_contracts::{
    CandidateModelId, ModelIntegrityState, ModelProviderArtifact, ProviderRegistrationResult,
    ProviderRegistrationState, RUNTIME_REPORT_SCHEMA_VERSION, RuntimeCapabilityAvailability,
    RuntimeCapabilityDescriptor, RuntimeCapabilityKind, RuntimeDisplayName, RuntimeEndpointSafety,
    RuntimeModelInventory, RuntimeModelMappingStatus, RuntimeModelSummary, RuntimeOperationKind,
    RuntimeProviderId, RuntimeProviderModelId, RuntimeProviderModelMapping, RuntimeReason,
    RuntimeReasonCode, RuntimeState, RuntimeVersionCompatibility, RuntimeVersionInfo,
    RuntimeWarning, RuntimeWarningCode, SetupDestinationCategory,
};
use gixgiz_runtime::{
    ChatDeltaSender, ModelProgressSender, RuntimeCancellationSemantics, RuntimeChatProvider,
    RuntimeChatRequest, RuntimeChatRole, RuntimeDetector, RuntimeError, RuntimeFuture,
    RuntimeGenerationResult, RuntimeLifecycle, RuntimeModelAcquisitionPlan,
    RuntimeModelAcquisitionResult, RuntimeModelAcquisitionStatus, RuntimeModelInspection,
    RuntimeModelInventoryProvider, RuntimeModelSetupProvider, RuntimeObservation,
    RuntimeOperationContext, RuntimeProvider, RuntimeReadinessInferenceResult,
    RuntimeStorageAvailability, RuntimeStoragePreflight,
};
use tracing::Instrument;

use crate::{
    chat_http::{ChatHttpLimits, HyperLoopbackChatHttpClient, OllamaChatHttpClient},
    discovery::{ExecutableLocator, ValidatedExecutable, WindowsExecutableLocator},
    endpoint::OLLAMA_HOST_ENV,
    endpoint::ValidatedEndpoint,
    error::OllamaAdapterError,
    http::{HttpLimits, HyperLoopbackHttpClient, OllamaHttpClient, OllamaRoute},
    model_http::{HyperLoopbackModelHttpClient, ModelHttpLimits, OllamaModelHttpClient},
    models::{MappedModel, map_model, provider_tag_for_candidate},
    process::{OwnedProcessControl, OwnedProcessStatus, ProcessLimits, TokioOwnedProcessControl},
    protocol::{ChatRequestMessage, TagModel, decode_tags, decode_version},
    storage::{FsModelStorageProbe, ModelStorageProbe},
    version::{VersionPolicy, VersionSupport},
};

/// Stable provider identifier registered by the v0.1 composition root.
pub const OLLAMA_PROVIDER_ID: &str = "gixgiz.runtime.ollama.v1";
const DISPLAY_NAME: &str = "Ollama";
const VERSION_BODY_LIMIT: usize = 4 * 1024;
const TAGS_BODY_LIMIT: usize = 1024 * 1024;
const COMMAND_OUTPUT_LIMIT: usize = 4 * 1024;
const MAX_PROVIDER_MODELS: usize = 1024;
const MAX_RETURNED_MODELS: usize = 256;
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);
const PROCESS_TIMEOUT: Duration = Duration::from_secs(3);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);
const POST_COMMIT_OBSERVATION_TIMEOUT: Duration = Duration::from_secs(7);
const START_POLL_INTERVAL: Duration = Duration::from_millis(100);
const MODEL_STORAGE_DISPLAY: &str = "Provider-managed model storage";
const MODEL_SOURCE_SUMMARY: &str = "Allowlisted provider model mapping.";
const MODEL_PULL_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const MODEL_PULL_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MODEL_PULL_BODY_LIMIT: usize = 8 * 1024 * 1024;
const MODEL_PULL_LINE_LIMIT: usize = 16 * 1024;
const MODEL_PULL_EVENT_LIMIT: usize = 100_000;
const READINESS_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const READINESS_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const READINESS_BODY_LIMIT: usize = 64 * 1024;

/// Concrete v0.1 adapter for a loopback-only local Ollama runtime.
pub struct OllamaAdapter {
    provider_id: RuntimeProviderId,
    endpoint: EndpointSelection,
    locator: Arc<dyn ExecutableLocator>,
    http: Arc<dyn OllamaHttpClient>,
    model_http: Arc<dyn OllamaModelHttpClient>,
    chat_http: Arc<dyn OllamaChatHttpClient>,
    storage: Arc<dyn ModelStorageProbe>,
    processes: Arc<dyn OwnedProcessControl>,
    clock: Arc<dyn Clock>,
    version_policy: VersionPolicy,
    starting: AtomicBool,
    acquiring: AtomicBool,
}

impl OllamaAdapter {
    /// Builds the non-failing current-user provider used by host composition.
    ///
    /// Missing or unsafe discovery evidence is reported by `detect` rather than preventing the
    /// supervised sidecar from starting.
    pub fn for_current_user() -> Result<Self, RuntimeError> {
        Ok(Self::from_environment(None))
    }

    /// Builds the production adapter from an optional caller-approved executable path and the
    /// current process's `OLLAMA_HOST` value.
    #[must_use]
    pub fn from_environment(explicit_executable: Option<PathBuf>) -> Self {
        let endpoint = match env::var(OLLAMA_HOST_ENV) {
            Ok(value) => EndpointSelection::from_value(Some(&value)),
            Err(env::VarError::NotPresent) => EndpointSelection::from_value(None),
            Err(env::VarError::NotUnicode(_)) => EndpointSelection::Rejected,
        };
        Self {
            provider_id: RuntimeProviderId::new(OLLAMA_PROVIDER_ID),
            endpoint,
            locator: Arc::new(WindowsExecutableLocator::new(explicit_executable)),
            http: Arc::new(HyperLoopbackHttpClient),
            model_http: Arc::new(HyperLoopbackModelHttpClient),
            chat_http: Arc::new(HyperLoopbackChatHttpClient),
            storage: Arc::new(FsModelStorageProbe::default()),
            processes: TokioOwnedProcessControl::shared(),
            clock: Arc::new(SystemClock),
            version_policy: VersionPolicy::v0_1(),
            starting: AtomicBool::new(false),
            acquiring: AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    fn with_components(
        endpoint: EndpointSelection,
        locator: Arc<dyn ExecutableLocator>,
        http: Arc<dyn OllamaHttpClient>,
        processes: Arc<dyn OwnedProcessControl>,
        clock: Arc<dyn Clock>,
        version_policy: VersionPolicy,
    ) -> Self {
        Self {
            provider_id: RuntimeProviderId::new(OLLAMA_PROVIDER_ID),
            endpoint,
            locator,
            http,
            model_http: Arc::new(HyperLoopbackModelHttpClient),
            chat_http: Arc::new(HyperLoopbackChatHttpClient),
            storage: Arc::new(FsModelStorageProbe::default()),
            processes,
            clock,
            version_policy,
            starting: AtomicBool::new(false),
            acquiring: AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    fn with_model_setup_components(
        mut self,
        model_http: Arc<dyn OllamaModelHttpClient>,
        storage: Arc<dyn ModelStorageProbe>,
    ) -> Self {
        self.model_http = model_http;
        self.storage = storage;
        self
    }

    async fn detect_inner(
        &self,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeObservation, RuntimeError> {
        context.check()?;
        let endpoint = match &self.endpoint {
            EndpointSelection::Valid(endpoint) => endpoint,
            EndpointSelection::Rejected => return Ok(self.unsafe_endpoint_observation()),
        };
        let executable = match self.locate(context).await {
            Ok(executable) => executable,
            Err(error) if operation_abort(&error) => return Err(error.into()),
            Err(_) => {
                return Ok(self.observation(
                    RuntimeState::Degraded,
                    RuntimeEndpointSafety::LoopbackVerified,
                    None,
                    false,
                    OwnedProcessStatus::None,
                    vec![reason(
                        RuntimeReasonCode::EvidenceIncomplete,
                        "Runtime installation evidence could not be validated.",
                    )],
                    vec![warning(
                        RuntimeWarningCode::PartialEvidence,
                        "Some local runtime evidence remains unavailable.",
                    )],
                ));
            }
        };
        let owned_status = match run_bounded(context, self.processes.owned_status()).await {
            Ok(status) => status,
            Err(error) if operation_abort(&error) => return Err(error.into()),
            Err(_) => {
                return Ok(self.observation(
                    RuntimeState::Degraded,
                    RuntimeEndpointSafety::LoopbackVerified,
                    None,
                    executable.is_some(),
                    OwnedProcessStatus::None,
                    vec![reason(
                        RuntimeReasonCode::EvidenceIncomplete,
                        "Managed runtime process state could not be verified.",
                    )],
                    vec![warning(
                        RuntimeWarningCode::PartialEvidence,
                        "Some local runtime evidence remains unavailable.",
                    )],
                ));
            }
        };

        match self.probe_server(context, endpoint).await {
            Ok(evidence) => Ok(self.running_observation(owned_status, evidence)),
            Err(error) if operation_abort(&error) => Err(error.into()),
            Err(OllamaAdapterError::ConnectionFailed) => {
                self.stopped_or_absent(context, endpoint, executable, owned_status)
                    .await
            }
            Err(OllamaAdapterError::HttpStatus { status: 404 }) => Ok(self.observation(
                RuntimeState::Incompatible,
                RuntimeEndpointSafety::LoopbackVerified,
                None,
                executable.is_some(),
                owned_status,
                vec![reason(
                    RuntimeReasonCode::VersionIncompatible,
                    "The local runtime does not expose the required provider capabilities.",
                )],
                vec![],
            )),
            Err(_) => Ok(self.observation(
                RuntimeState::Degraded,
                RuntimeEndpointSafety::LoopbackVerified,
                None,
                executable.is_some(),
                owned_status,
                vec![reason(
                    RuntimeReasonCode::EvidenceIncomplete,
                    "The local runtime returned incomplete health evidence.",
                )],
                vec![warning(
                    RuntimeWarningCode::PartialEvidence,
                    "Some local runtime evidence remains unavailable.",
                )],
            )),
        }
    }

    async fn stopped_or_absent(
        &self,
        context: &RuntimeOperationContext,
        endpoint: &ValidatedEndpoint,
        executable: Option<ValidatedExecutable>,
        owned_status: OwnedProcessStatus,
    ) -> Result<RuntimeObservation, RuntimeError> {
        if owned_status == OwnedProcessStatus::Exited {
            return Ok(self.observation(
                RuntimeState::Failed,
                RuntimeEndpointSafety::LoopbackVerified,
                None,
                executable.is_some(),
                owned_status,
                vec![
                    reason(
                        RuntimeReasonCode::ProcessExited,
                        "The managed runtime process exited before health could be verified.",
                    ),
                    reason(
                        RuntimeReasonCode::EndpointUnavailable,
                        "The approved local runtime endpoint is unavailable.",
                    ),
                ],
                vec![],
            ));
        }
        if owned_status == OwnedProcessStatus::Running {
            let state = if self.starting.load(Ordering::Acquire) {
                RuntimeState::Starting
            } else {
                RuntimeState::Degraded
            };
            return Ok(self.observation(
                state,
                RuntimeEndpointSafety::LoopbackVerified,
                None,
                executable.is_some(),
                owned_status,
                vec![reason(
                    RuntimeReasonCode::EvidenceIncomplete,
                    "The managed runtime process is active, but health is not yet verified.",
                )],
                vec![warning(
                    RuntimeWarningCode::PartialEvidence,
                    "The managed runtime process is not currently healthy.",
                )],
            ));
        }
        let Some(executable) = executable else {
            return Ok(self.observation(
                RuntimeState::NotInstalled,
                RuntimeEndpointSafety::LoopbackVerified,
                None,
                false,
                owned_status,
                vec![
                    reason(
                        RuntimeReasonCode::InstallationNotFound,
                        "No validated local runtime installation was found.",
                    ),
                    reason(
                        RuntimeReasonCode::EndpointUnavailable,
                        "The approved local runtime endpoint is unavailable.",
                    ),
                ],
                vec![],
            ));
        };

        let limits = ProcessLimits {
            timeout: PROCESS_TIMEOUT.min(context.remaining()),
            max_output_bytes: COMMAND_OUTPUT_LIMIT,
        };
        match run_bounded(
            context,
            self.processes.client_version(&executable, endpoint, limits),
        )
        .await
        {
            Ok(version) => {
                let raw = version.to_string();
                let (_, support) = self
                    .version_policy
                    .assess(&raw)
                    .map_err(RuntimeError::from)?;
                let state = match support {
                    VersionSupport::Compatible => RuntimeState::InstalledStopped,
                    VersionSupport::Untested => RuntimeState::Degraded,
                    VersionSupport::Incompatible => RuntimeState::Incompatible,
                };
                Ok(self.observation(
                    state,
                    RuntimeEndpointSafety::LoopbackVerified,
                    Some(version_info(raw, support)),
                    true,
                    owned_status,
                    vec![
                        reason(
                            RuntimeReasonCode::ExecutableVerified,
                            "A validated local runtime executable was found.",
                        ),
                        reason(
                            RuntimeReasonCode::EndpointUnavailable,
                            "The approved local runtime endpoint is unavailable.",
                        ),
                    ],
                    version_warnings(support),
                ))
            }
            Err(OllamaAdapterError::ExecutableUntrusted) => Ok(self.observation(
                RuntimeState::Degraded,
                RuntimeEndpointSafety::LoopbackVerified,
                None,
                false,
                owned_status,
                vec![reason(
                    RuntimeReasonCode::EvidenceIncomplete,
                    "The runtime executable signature and publisher could not be trusted.",
                )],
                vec![warning(
                    RuntimeWarningCode::PartialEvidence,
                    "The installation cannot be managed safely until executable trust is verified.",
                )],
            )),
            Err(error) if operation_abort(&error) => Err(error.into()),
            Err(_) => Ok(self.observation(
                RuntimeState::Degraded,
                RuntimeEndpointSafety::LoopbackVerified,
                None,
                false,
                owned_status,
                vec![
                    reason(
                        RuntimeReasonCode::EvidenceIncomplete,
                        "Executable trust evidence could not be completed.",
                    ),
                    reason(
                        RuntimeReasonCode::VersionUnverified,
                        "The stopped runtime version could not be verified.",
                    ),
                ],
                vec![warning(
                    RuntimeWarningCode::PartialEvidence,
                    "The installation is present, but its compatibility is unknown.",
                )],
            )),
        }
    }

    async fn locate(
        &self,
        context: &RuntimeOperationContext,
    ) -> Result<Option<ValidatedExecutable>, OllamaAdapterError> {
        let locator = Arc::clone(&self.locator);
        run_bounded(context, async move {
            tokio::task::spawn_blocking(move || locator.locate())
                .await
                .map_err(|_| OllamaAdapterError::Internal)?
        })
        .await
    }

    async fn probe_server(
        &self,
        context: &RuntimeOperationContext,
        endpoint: &ValidatedEndpoint,
    ) -> Result<ServerEvidence, OllamaAdapterError> {
        let version_body = run_bounded(
            context,
            self.http.get(
                endpoint,
                OllamaRoute::Version,
                http_limits(context, VERSION_BODY_LIMIT),
            ),
        )
        .await?;
        let response = decode_version(&version_body)?;
        let raw_version = response.version.trim().to_owned();
        let (version, support) = self.version_policy.assess(&raw_version)?;
        if support == VersionSupport::Incompatible {
            return Ok(ServerEvidence {
                raw_version,
                normalized_version: version,
                support,
                tags: vec![],
            });
        }

        let tags_body = run_bounded(
            context,
            self.http.get(
                endpoint,
                OllamaRoute::Tags,
                http_limits(context, TAGS_BODY_LIMIT),
            ),
        )
        .await?;
        let tags = decode_tags(&tags_body, MAX_PROVIDER_MODELS)?.models;
        Ok(ServerEvidence {
            raw_version,
            normalized_version: version,
            support,
            tags,
        })
    }

    fn running_observation(
        &self,
        owned_status: OwnedProcessStatus,
        evidence: ServerEvidence,
    ) -> RuntimeObservation {
        let state = match evidence.support {
            VersionSupport::Compatible => RuntimeState::Ready,
            VersionSupport::Untested => RuntimeState::Degraded,
            VersionSupport::Incompatible => RuntimeState::Incompatible,
        };
        let reasons = vec![
            reason(
                RuntimeReasonCode::EndpointReachable,
                "The approved local runtime endpoint returned valid health evidence.",
            ),
            match evidence.support {
                VersionSupport::Compatible => reason(
                    RuntimeReasonCode::VersionCompatible,
                    "The runtime exposes the required local capabilities.",
                ),
                VersionSupport::Untested => reason(
                    RuntimeReasonCode::VersionUnverified,
                    "The runtime exposes required capabilities, but this version is untested.",
                ),
                VersionSupport::Incompatible => reason(
                    RuntimeReasonCode::VersionIncompatible,
                    "The runtime version is outside the supported capability policy.",
                ),
            },
        ];
        self.observation(
            state,
            RuntimeEndpointSafety::LoopbackVerified,
            Some(RuntimeVersionInfo {
                reported_version: evidence.raw_version,
                normalized_version: Some(evidence.normalized_version.to_string()),
                compatibility: compatibility(evidence.support),
            }),
            false,
            owned_status,
            reasons,
            version_warnings(evidence.support),
        )
    }

    fn unsafe_endpoint_observation(&self) -> RuntimeObservation {
        self.observation(
            RuntimeState::Degraded,
            RuntimeEndpointSafety::Unsafe,
            None,
            false,
            OwnedProcessStatus::None,
            vec![reason(
                RuntimeReasonCode::EndpointUnsafe,
                "The configured runtime endpoint is not restricted to this computer.",
            )],
            vec![warning(
                RuntimeWarningCode::EndpointExposure,
                "Restore a loopback-only runtime endpoint before reuse.",
            )],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn observation(
        &self,
        state: RuntimeState,
        endpoint_safety: RuntimeEndpointSafety,
        version: Option<RuntimeVersionInfo>,
        executable_found: bool,
        owned_status: OwnedProcessStatus,
        reasons: Vec<RuntimeReason>,
        warnings: Vec<RuntimeWarning>,
    ) -> RuntimeObservation {
        let version_verified = version.is_some();
        let provider_capabilities_verified = state != RuntimeState::Incompatible
            && reasons
                .iter()
                .any(|reason| reason.code == RuntimeReasonCode::EndpointReachable);
        RuntimeObservation {
            provider_id: self.provider_id.clone(),
            display_name: RuntimeDisplayName::new(DISPLAY_NAME),
            state,
            endpoint_safety,
            version,
            capabilities: capabilities(
                state,
                endpoint_safety,
                version_verified,
                provider_capabilities_verified,
                executable_found,
                owned_status,
            ),
            reasons,
            warnings,
        }
    }

    async fn execute_start(
        &self,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeObservation, RuntimeError> {
        let endpoint = self.valid_endpoint()?;
        match run_bounded(context, self.processes.owned_status()).await? {
            OwnedProcessStatus::Running => return Err(RuntimeError::Busy),
            OwnedProcessStatus::None | OwnedProcessStatus::Exited => {}
        }

        match run_bounded(
            context,
            self.http.get(
                endpoint,
                OllamaRoute::Version,
                http_limits(context, VERSION_BODY_LIMIT),
            ),
        )
        .await
        {
            Ok(_) => return Err(RuntimeError::OwnershipConflict),
            Err(OllamaAdapterError::ConnectionFailed) => {}
            Err(OllamaAdapterError::HttpStatus { status: 404 }) => {
                return Err(RuntimeError::IncompatibleVersion);
            }
            Err(error) => return Err(error.into()),
        }

        let executable = self
            .locate(context)
            .await?
            .ok_or(RuntimeError::NotInstalled)?;
        let client_version = run_bounded(
            context,
            self.processes.client_version(
                &executable,
                endpoint,
                ProcessLimits {
                    timeout: PROCESS_TIMEOUT.min(context.remaining()),
                    max_output_bytes: COMMAND_OUTPUT_LIMIT,
                },
            ),
        )
        .await?;
        let (_, support) = self.version_policy.assess(&client_version.to_string())?;
        if support != VersionSupport::Compatible {
            return Err(RuntimeError::IncompatibleVersion);
        }

        run_committed(context, self.processes.start_owned(&executable, endpoint)).await?;
        self.starting.store(true, Ordering::Release);
        let _starting_guard = StartingGuard(&self.starting);
        loop {
            if let Err(error) = context.check() {
                self.rollback_owned().await;
                return Err(error);
            }
            let owned_status = match run_bounded(context, self.processes.owned_status()).await {
                Ok(status) => status,
                Err(error) => {
                    self.rollback_owned().await;
                    return Err(error.into());
                }
            };
            match owned_status {
                OwnedProcessStatus::Running => {}
                OwnedProcessStatus::Exited | OwnedProcessStatus::None => {
                    return Err(RuntimeError::ProcessFailed);
                }
            }
            match self.probe_server(context, endpoint).await {
                Ok(evidence) if evidence.support == VersionSupport::Incompatible => {
                    self.rollback_owned().await;
                    return Err(RuntimeError::IncompatibleVersion);
                }
                Ok(evidence) => {
                    return Ok(self.running_observation(OwnedProcessStatus::Running, evidence));
                }
                Err(OllamaAdapterError::ConnectionFailed) => {}
                Err(OllamaAdapterError::HttpStatus { status: 404 }) => {
                    self.rollback_owned().await;
                    return Err(RuntimeError::IncompatibleVersion);
                }
                Err(error) if operation_abort(&error) => {
                    self.rollback_owned().await;
                    return Err(error.into());
                }
                Err(error) => {
                    self.rollback_owned().await;
                    return Err(error.into());
                }
            }
            if let Err(error) = run_bounded(context, async {
                tokio::time::sleep(START_POLL_INTERVAL).await;
                Ok(())
            })
            .await
            {
                self.rollback_owned().await;
                return Err(error.into());
            }
        }
    }

    async fn execute_stop(
        &self,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeObservation, RuntimeError> {
        self.valid_endpoint()?;
        self.commit_owned_stop(context).await?;
        Ok(self.observe_after_committed_stop(context).await)
    }

    async fn execute_restart(
        &self,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeObservation, RuntimeError> {
        self.valid_endpoint()?;
        self.commit_owned_stop(context).await?;
        context.check()?;
        self.execute_start(context).await
    }

    async fn commit_owned_stop(
        &self,
        context: &RuntimeOperationContext,
    ) -> Result<(), RuntimeError> {
        if run_bounded(context, self.processes.owned_status()).await? != OwnedProcessStatus::Running
        {
            return Err(RuntimeError::OwnershipConflict);
        }
        // The exact-child kill is the stop commit point. Cancellation is honored above; after
        // this point bounded cleanup completes so Stop can return authoritative post-effect state.
        let timeout = STOP_TIMEOUT.min(context.remaining());
        run_committed(context, self.processes.stop_owned(timeout)).await?;
        Ok(())
    }

    async fn observe_after_committed_stop(
        &self,
        original_context: &RuntimeOperationContext,
    ) -> RuntimeObservation {
        let observation_context = RuntimeOperationContext::new(
            original_context.correlation_id(),
            original_context.request_id(),
            POST_COMMIT_OBSERVATION_TIMEOUT,
        );
        self.detect_inner(&observation_context)
            .await
            .unwrap_or_else(|_| {
                self.observation(
                    RuntimeState::Degraded,
                    RuntimeEndpointSafety::LoopbackVerified,
                    None,
                    false,
                    OwnedProcessStatus::None,
                    vec![reason(
                        RuntimeReasonCode::EvidenceIncomplete,
                        "The owned runtime stopped, but follow-up evidence is incomplete.",
                    )],
                    vec![warning(
                        RuntimeWarningCode::PartialEvidence,
                        "Refresh runtime status before the next lifecycle action.",
                    )],
                )
            })
    }

    async fn rollback_owned(&self) {
        let _ = self.processes.stop_owned(STOP_TIMEOUT).await;
    }

    fn valid_endpoint(&self) -> Result<&ValidatedEndpoint, RuntimeError> {
        match &self.endpoint {
            EndpointSelection::Valid(endpoint) => Ok(endpoint),
            EndpointSelection::Rejected => Err(RuntimeError::EndpointUnsafe),
        }
    }

    async fn list_models_inner(
        &self,
        limit: u16,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeModelInventory, RuntimeError> {
        let endpoint = self.valid_endpoint()?;
        let evidence = match self.probe_server(context, endpoint).await {
            Ok(evidence) => evidence,
            Err(OllamaAdapterError::HttpStatus { status: 404 }) => {
                return Err(RuntimeError::IncompatibleVersion);
            }
            Err(error) => return Err(error.into()),
        };
        if evidence.support == VersionSupport::Incompatible {
            return Err(RuntimeError::IncompatibleVersion);
        }
        let requested_limit = usize::from(limit).min(MAX_RETURNED_MODELS);
        let mapped: Vec<MappedModel> = evidence.tags.into_iter().map(map_model).collect();
        let provider_count = mapped.len();
        let local_count = mapped.iter().filter(|model| !model.is_remote).count();
        let models = mapped
            .into_iter()
            .filter(|model| !model.is_remote)
            .take(requested_limit)
            .map(runtime_model_summary)
            .collect();

        Ok(RuntimeModelInventory {
            schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
            provider_id: self.provider_id.clone(),
            models,
            truncated: provider_count > local_count
                || local_count > requested_limit
                || usize::from(limit) > MAX_RETURNED_MODELS,
            collected_at_unix_ms: self.clock.unix_ms(),
        })
    }

    fn prepare_model_acquisition_inner(
        &self,
        model_id: CandidateModelId,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeModelAcquisitionPlan, RuntimeError> {
        context.check()?;
        let provider_tag =
            provider_tag_for_candidate(&model_id).ok_or(RuntimeError::ModelNotMapped)?;
        Ok(RuntimeModelAcquisitionPlan {
            artifact: ModelProviderArtifact {
                canonical_model_id: model_id,
                provider_id: self.provider_id.clone(),
                provider_model_id: RuntimeProviderModelId::new(provider_tag),
                source_summary: MODEL_SOURCE_SUMMARY.to_owned(),
            },
            destination: SetupDestinationCategory::ProviderManaged,
            destination_display: MODEL_STORAGE_DISPLAY.to_owned(),
            cancellation: RuntimeCancellationSemantics::ConnectionAbortMayRetainEffects,
        })
    }

    async fn preflight_model_storage_inner(
        &self,
        plan: RuntimeModelAcquisitionPlan,
        required_bytes: u64,
        safety_margin_bytes: u64,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeStoragePreflight, RuntimeError> {
        self.validate_acquisition_plan(&plan)?;
        let required_with_margin = required_bytes
            .checked_add(safety_margin_bytes)
            .ok_or(RuntimeError::InvalidInput)?;
        let storage = Arc::clone(&self.storage);
        let available_result = context
            .run(async move {
                tokio::task::spawn_blocking(move || storage.available_bytes())
                    .await
                    .map_err(|_| RuntimeError::Internal)?
                    .map_err(RuntimeError::from)
            })
            .await;
        let (availability, available_bytes) = match available_result {
            Ok(available) if available >= required_with_margin => {
                (RuntimeStorageAvailability::Available, Some(available))
            }
            Ok(available) => (
                RuntimeStorageAvailability::InsufficientSpace,
                Some(available),
            ),
            Err(RuntimeError::ModelStorageUnavailable) => {
                (RuntimeStorageAvailability::Unavailable, None)
            }
            Err(error) => return Err(error),
        };
        Ok(RuntimeStoragePreflight {
            destination: plan.destination,
            destination_display: plan.destination_display,
            availability,
            required_bytes,
            safety_margin_bytes,
            available_bytes,
            checked_at_unix_ms: self.clock.unix_ms(),
        })
    }

    async fn acquire_model_inner(
        &self,
        plan: RuntimeModelAcquisitionPlan,
        progress: ModelProgressSender,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeModelAcquisitionResult, RuntimeError> {
        let provider_tag = self.validate_acquisition_plan(&plan)?;
        let existing = self
            .inspect_model_inner(plan.artifact.clone(), context)
            .await?;
        if existing.available {
            return Ok(acquisition_result(
                existing,
                RuntimeModelAcquisitionStatus::AlreadyPresent,
                self.clock.unix_ms(),
            ));
        }
        if self
            .acquiring
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(RuntimeError::Busy);
        }
        let _guard = AcquiringGuard(&self.acquiring);

        // Recheck after obtaining the single-acquisition guard so a peer completion is reused.
        let existing = self
            .inspect_model_inner(plan.artifact.clone(), context)
            .await?;
        if existing.available {
            return Ok(acquisition_result(
                existing,
                RuntimeModelAcquisitionStatus::AlreadyPresent,
                self.clock.unix_ms(),
            ));
        }
        let endpoint = self.valid_endpoint()?;
        let pull_outcome = run_bounded(
            context,
            self.model_http
                .pull(endpoint, provider_tag, progress, model_pull_limits(context)),
        )
        .await
        .map_err(RuntimeError::from)?;
        let inspection = self.inspect_model_inner(plan.artifact, context).await?;
        if !inspection.available
            || inspection.registration.state != ProviderRegistrationState::Registered
        {
            return Err(RuntimeError::ModelRegistrationFailed);
        }
        let mut result = acquisition_result(
            inspection,
            RuntimeModelAcquisitionStatus::Acquired,
            self.clock.unix_ms(),
        );
        if pull_outcome.provider_integrity {
            result.integrity = ModelIntegrityState::ProviderReported;
        }
        Ok(result)
    }

    async fn inspect_model_inner(
        &self,
        artifact: ModelProviderArtifact,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeModelInspection, RuntimeError> {
        let provider_tag = self.validate_artifact(&artifact)?;
        let endpoint = self.valid_endpoint()?;
        let evidence = match self.probe_server(context, endpoint).await {
            Ok(evidence) => evidence,
            Err(OllamaAdapterError::HttpStatus { status: 404 }) => {
                return Err(RuntimeError::IncompatibleVersion);
            }
            Err(error) => return Err(error.into()),
        };
        if evidence.support == VersionSupport::Incompatible {
            return Err(RuntimeError::IncompatibleVersion);
        }
        let registered = evidence
            .tags
            .into_iter()
            .find(|model| !is_remote_model(model) && provider_model_name(model) == provider_tag);
        let (available, state, measured_size_bytes, integrity) = if let Some(model) = registered {
            (
                true,
                ProviderRegistrationState::Registered,
                Some(model.size),
                ModelIntegrityState::ProviderReported,
            )
        } else {
            (
                false,
                ProviderRegistrationState::NotRegistered,
                None,
                ModelIntegrityState::Unavailable,
            )
        };
        Ok(RuntimeModelInspection {
            artifact,
            available,
            registration: ProviderRegistrationResult {
                state,
                measured_size_bytes,
                verified_at_unix_ms: self.clock.unix_ms(),
            },
            integrity,
        })
    }

    async fn run_readiness_inference_inner(
        &self,
        artifact: ModelProviderArtifact,
        context: &RuntimeOperationContext,
    ) -> Result<RuntimeReadinessInferenceResult, RuntimeError> {
        let provider_tag = self.validate_artifact(&artifact)?;
        let inspection = self.inspect_model_inner(artifact, context).await?;
        if !inspection.available {
            return Err(RuntimeError::ModelUnavailable);
        }
        let endpoint = self.valid_endpoint()?;
        run_bounded(
            context,
            self.model_http
                .readiness(endpoint, provider_tag, readiness_limits(context)),
        )
        .await
        .map_err(RuntimeError::from)?;
        Ok(RuntimeReadinessInferenceResult {
            ready: true,
            completed_at_unix_ms: self.clock.unix_ms(),
        })
    }

    fn validate_acquisition_plan(
        &self,
        plan: &RuntimeModelAcquisitionPlan,
    ) -> Result<&'static str, RuntimeError> {
        if plan.destination != SetupDestinationCategory::ProviderManaged
            || plan.destination_display != MODEL_STORAGE_DISPLAY
            || plan.cancellation != RuntimeCancellationSemantics::ConnectionAbortMayRetainEffects
        {
            return Err(RuntimeError::InvalidInput);
        }
        self.validate_artifact(&plan.artifact)
    }

    fn validate_artifact(
        &self,
        artifact: &ModelProviderArtifact,
    ) -> Result<&'static str, RuntimeError> {
        if artifact.provider_id != self.provider_id {
            return Err(RuntimeError::InvalidInput);
        }
        let provider_tag = provider_tag_for_candidate(&artifact.canonical_model_id)
            .ok_or(RuntimeError::ModelNotMapped)?;
        if artifact.provider_model_id.as_str() != provider_tag
            || artifact.source_summary != MODEL_SOURCE_SUMMARY
        {
            return Err(RuntimeError::InvalidInput);
        }
        Ok(provider_tag)
    }
}

impl Default for OllamaAdapter {
    fn default() -> Self {
        Self::from_environment(None)
    }
}

impl RuntimeDetector for OllamaAdapter {
    fn detect(&self, context: RuntimeOperationContext) -> RuntimeFuture<'_, RuntimeObservation> {
        let span = tracing::info_span!(
            "runtime_provider_detect",
            provider_id = OLLAMA_PROVIDER_ID,
            correlation_id = %context.correlation_id(),
            request_id = %context.request_id(),
        );
        Box::pin(
            async move {
                let result = self.detect_inner(&context).await;
                match &result {
                    Ok(observation) => tracing::info!(
                        state = ?observation.state,
                        "runtime provider detection completed"
                    ),
                    Err(error) => tracing::warn!(
                        error_code = ?error.code(),
                        "runtime provider detection failed"
                    ),
                }
                result
            }
            .instrument(span),
        )
    }
}

impl RuntimeLifecycle for OllamaAdapter {
    fn execute(
        &self,
        kind: RuntimeOperationKind,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeObservation> {
        let span = tracing::info_span!(
            "runtime_provider_lifecycle",
            provider_id = OLLAMA_PROVIDER_ID,
            operation_kind = ?kind,
            correlation_id = %context.correlation_id(),
            request_id = %context.request_id(),
        );
        Box::pin(
            async move {
                let result = match kind {
                    RuntimeOperationKind::Start => self.execute_start(&context).await,
                    RuntimeOperationKind::Stop => self.execute_stop(&context).await,
                    RuntimeOperationKind::Restart => self.execute_restart(&context).await,
                    RuntimeOperationKind::Unknown => Err(RuntimeError::Unsupported),
                    _ => Err(RuntimeError::Unsupported),
                };
                match &result {
                    Ok(observation) => tracing::info!(
                        state = ?observation.state,
                        "runtime provider lifecycle completed"
                    ),
                    Err(error) => tracing::warn!(
                        error_code = ?error.code(),
                        "runtime provider lifecycle failed"
                    ),
                }
                result
            }
            .instrument(span),
        )
    }
}

impl RuntimeModelInventoryProvider for OllamaAdapter {
    fn list_models(
        &self,
        limit: u16,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelInventory> {
        let span = tracing::info_span!(
            "runtime_provider_model_inventory",
            provider_id = OLLAMA_PROVIDER_ID,
            requested_limit = limit,
            correlation_id = %context.correlation_id(),
            request_id = %context.request_id(),
        );
        Box::pin(
            async move {
                let result = self.list_models_inner(limit, &context).await;
                match &result {
                    Ok(inventory) => tracing::info!(
                        model_count = inventory.models.len(),
                        truncated = inventory.truncated,
                        "runtime provider model inventory completed"
                    ),
                    Err(error) => tracing::warn!(
                        error_code = ?error.code(),
                        "runtime provider model inventory failed"
                    ),
                }
                result
            }
            .instrument(span),
        )
    }
}

impl RuntimeModelSetupProvider for OllamaAdapter {
    fn prepare_model_acquisition(
        &self,
        model_id: CandidateModelId,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelAcquisitionPlan> {
        let span = tracing::info_span!(
            "runtime_provider_model_prepare",
            provider_id = OLLAMA_PROVIDER_ID,
            correlation_id = %context.correlation_id(),
            request_id = %context.request_id(),
        );
        Box::pin(
            async move { self.prepare_model_acquisition_inner(model_id, &context) }
                .instrument(span),
        )
    }

    fn preflight_model_storage(
        &self,
        plan: RuntimeModelAcquisitionPlan,
        required_bytes: u64,
        safety_margin_bytes: u64,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeStoragePreflight> {
        let span = tracing::info_span!(
            "runtime_provider_model_storage_preflight",
            provider_id = OLLAMA_PROVIDER_ID,
            correlation_id = %context.correlation_id(),
            request_id = %context.request_id(),
        );
        Box::pin(
            async move {
                self.preflight_model_storage_inner(
                    plan,
                    required_bytes,
                    safety_margin_bytes,
                    &context,
                )
                .await
            }
            .instrument(span),
        )
    }

    fn acquire_model(
        &self,
        plan: RuntimeModelAcquisitionPlan,
        progress: ModelProgressSender,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelAcquisitionResult> {
        let span = tracing::info_span!(
            "runtime_provider_model_acquisition",
            provider_id = OLLAMA_PROVIDER_ID,
            correlation_id = %context.correlation_id(),
            request_id = %context.request_id(),
        );
        Box::pin(
            async move {
                let result = self.acquire_model_inner(plan, progress, &context).await;
                match &result {
                    Ok(result) => tracing::info!(
                        acquisition_status = ?result.status,
                        "runtime provider model acquisition completed"
                    ),
                    Err(error) => tracing::warn!(
                        error_code = ?error.code(),
                        "runtime provider model acquisition stopped"
                    ),
                }
                result
            }
            .instrument(span),
        )
    }

    fn inspect_model(
        &self,
        artifact: ModelProviderArtifact,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelInspection> {
        let span = tracing::info_span!(
            "runtime_provider_model_inspection",
            provider_id = OLLAMA_PROVIDER_ID,
            correlation_id = %context.correlation_id(),
            request_id = %context.request_id(),
        );
        Box::pin(async move { self.inspect_model_inner(artifact, &context).await }.instrument(span))
    }

    fn run_readiness_inference(
        &self,
        artifact: ModelProviderArtifact,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeReadinessInferenceResult> {
        let span = tracing::info_span!(
            "runtime_provider_model_readiness",
            provider_id = OLLAMA_PROVIDER_ID,
            correlation_id = %context.correlation_id(),
            request_id = %context.request_id(),
        );
        Box::pin(
            async move {
                let result = self.run_readiness_inference_inner(artifact, &context).await;
                if let Err(error) = &result {
                    tracing::warn!(
                        error_code = ?error.code(),
                        "runtime provider model readiness failed"
                    );
                }
                result
            }
            .instrument(span),
        )
    }
}

impl RuntimeProvider for OllamaAdapter {
    fn provider_id(&self) -> &RuntimeProviderId {
        &self.provider_id
    }
}

#[derive(Clone)]
enum EndpointSelection {
    Valid(ValidatedEndpoint),
    Rejected,
}

impl EndpointSelection {
    fn from_value(value: Option<&str>) -> Self {
        match ValidatedEndpoint::from_ollama_host(value) {
            Ok(endpoint) => Self::Valid(endpoint),
            Err(_) => Self::Rejected,
        }
    }
}

struct ServerEvidence {
    raw_version: String,
    normalized_version: semver::Version,
    support: VersionSupport,
    tags: Vec<TagModel>,
}

trait Clock: Send + Sync {
    fn unix_ms(&self) -> u64;
}

struct SystemClock;

impl Clock for SystemClock {
    fn unix_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                duration.as_millis().try_into().unwrap_or(u64::MAX)
            })
    }
}

struct StartingGuard<'a>(&'a AtomicBool);

impl Drop for StartingGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct AcquiringGuard<'a>(&'a AtomicBool);

impl Drop for AcquiringGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn model_pull_limits(context: &RuntimeOperationContext) -> ModelHttpLimits {
    ModelHttpLimits {
        connect_timeout: MODEL_PULL_CONNECT_TIMEOUT.min(context.remaining()),
        idle_timeout: MODEL_PULL_IDLE_TIMEOUT.min(context.remaining()),
        max_body_bytes: MODEL_PULL_BODY_LIMIT,
        max_line_bytes: MODEL_PULL_LINE_LIMIT,
        max_events: MODEL_PULL_EVENT_LIMIT,
    }
}

fn readiness_limits(context: &RuntimeOperationContext) -> ModelHttpLimits {
    ModelHttpLimits {
        connect_timeout: READINESS_CONNECT_TIMEOUT.min(context.remaining()),
        idle_timeout: READINESS_IDLE_TIMEOUT.min(context.remaining()),
        max_body_bytes: READINESS_BODY_LIMIT,
        max_line_bytes: READINESS_BODY_LIMIT,
        max_events: 1,
    }
}

fn provider_model_name(model: &TagModel) -> &str {
    if model.model.trim().is_empty() {
        model.name.as_str()
    } else {
        model.model.as_str()
    }
}

fn is_remote_model(model: &TagModel) -> bool {
    !model.remote_model.trim().is_empty() || !model.remote_host.trim().is_empty()
}

fn acquisition_result(
    inspection: RuntimeModelInspection,
    status: RuntimeModelAcquisitionStatus,
    completed_at_unix_ms: u64,
) -> RuntimeModelAcquisitionResult {
    RuntimeModelAcquisitionResult {
        artifact: inspection.artifact,
        status,
        measured_size_bytes: inspection.registration.measured_size_bytes,
        integrity: inspection.integrity,
        completed_at_unix_ms,
    }
}

async fn run_bounded<T>(
    context: &RuntimeOperationContext,
    future: impl Future<Output = Result<T, OllamaAdapterError>>,
) -> Result<T, OllamaAdapterError> {
    context.check().map_err(adapter_context_error)?;
    let cancellation = context.cancellation();
    tokio::select! {
        biased;
        () = cancellation.cancelled() => Err(OllamaAdapterError::Cancelled),
        result = tokio::time::timeout(context.remaining(), future) => {
            result.unwrap_or(Err(OllamaAdapterError::TimedOut))
        }
    }
}

/// Runs a bounded mutation after one final cancellation check.
///
/// Once polling begins, the caller decides how to handle cancellation after the mutation returns,
/// so the adapter never abandons an exact-child handle at an ambiguous process commit point.
async fn run_committed<T>(
    context: &RuntimeOperationContext,
    future: impl Future<Output = Result<T, OllamaAdapterError>>,
) -> Result<T, OllamaAdapterError> {
    context.check().map_err(adapter_context_error)?;
    tokio::time::timeout(context.remaining(), future)
        .await
        .unwrap_or(Err(OllamaAdapterError::TimedOut))
}

fn adapter_context_error(error: RuntimeError) -> OllamaAdapterError {
    match error {
        RuntimeError::Cancelled => OllamaAdapterError::Cancelled,
        RuntimeError::TimedOut => OllamaAdapterError::TimedOut,
        _ => OllamaAdapterError::Internal,
    }
}

fn operation_abort(error: &OllamaAdapterError) -> bool {
    matches!(
        error,
        OllamaAdapterError::Cancelled | OllamaAdapterError::TimedOut
    )
}

fn http_limits(context: &RuntimeOperationContext, max_body_bytes: usize) -> HttpLimits {
    HttpLimits {
        timeout: HTTP_TIMEOUT.min(context.remaining()),
        max_body_bytes,
    }
}

fn version_info(raw: String, support: VersionSupport) -> RuntimeVersionInfo {
    let normalized_version = semver::Version::parse(&raw)
        .ok()
        .map(|value| value.to_string());
    RuntimeVersionInfo {
        reported_version: raw,
        normalized_version,
        compatibility: compatibility(support),
    }
}

const fn compatibility(support: VersionSupport) -> RuntimeVersionCompatibility {
    match support {
        VersionSupport::Compatible => RuntimeVersionCompatibility::Compatible,
        VersionSupport::Untested => RuntimeVersionCompatibility::Untested,
        VersionSupport::Incompatible => RuntimeVersionCompatibility::Incompatible,
    }
}

fn version_warnings(support: VersionSupport) -> Vec<RuntimeWarning> {
    if support == VersionSupport::Untested {
        vec![warning(
            RuntimeWarningCode::VersionUntested,
            "The runtime version is valid but lacks recorded real-provider evidence.",
        )]
    } else {
        vec![]
    }
}

fn capabilities(
    state: RuntimeState,
    endpoint_safety: RuntimeEndpointSafety,
    version_verified: bool,
    provider_capabilities_verified: bool,
    executable_found: bool,
    owned_status: OwnedProcessStatus,
) -> Vec<RuntimeCapabilityDescriptor> {
    let endpoint_available = endpoint_safety == RuntimeEndpointSafety::LoopbackVerified;
    let health_availability = if endpoint_available && provider_capabilities_verified {
        RuntimeCapabilityAvailability::Available
    } else {
        RuntimeCapabilityAvailability::Unsupported
    };
    let version_availability = if endpoint_available && version_verified {
        RuntimeCapabilityAvailability::Available
    } else {
        RuntimeCapabilityAvailability::Unsupported
    };
    let start_availability =
        if endpoint_available && executable_found && state == RuntimeState::InstalledStopped {
            RuntimeCapabilityAvailability::Available
        } else {
            RuntimeCapabilityAvailability::Unsupported
        };
    let owned_availability = if endpoint_available && owned_status == OwnedProcessStatus::Running {
        RuntimeCapabilityAvailability::Available
    } else {
        RuntimeCapabilityAvailability::Unsupported
    };
    let setup_approval_availability =
        if health_availability == RuntimeCapabilityAvailability::Available {
            RuntimeCapabilityAvailability::RequiresSetupApproval
        } else {
            RuntimeCapabilityAvailability::Unsupported
        };

    vec![
        capability(
            RuntimeCapabilityKind::Detection,
            RuntimeCapabilityAvailability::Available,
        ),
        capability(RuntimeCapabilityKind::Health, health_availability),
        capability(RuntimeCapabilityKind::Version, version_availability),
        capability(RuntimeCapabilityKind::Start, start_availability),
        capability(RuntimeCapabilityKind::Stop, owned_availability),
        capability(RuntimeCapabilityKind::Restart, owned_availability),
        capability(RuntimeCapabilityKind::ModelInventory, health_availability),
        capability(
            RuntimeCapabilityKind::ModelAcquisitionPreparation,
            health_availability,
        ),
        capability(
            RuntimeCapabilityKind::ModelStoragePreflight,
            health_availability,
        ),
        capability(
            RuntimeCapabilityKind::ModelAcquisition,
            setup_approval_availability,
        ),
        capability(
            RuntimeCapabilityKind::ModelRegistration,
            setup_approval_availability,
        ),
        capability(
            RuntimeCapabilityKind::ReadinessInference,
            setup_approval_availability,
        ),
    ]
}

fn capability(
    kind: RuntimeCapabilityKind,
    availability: RuntimeCapabilityAvailability,
) -> RuntimeCapabilityDescriptor {
    RuntimeCapabilityDescriptor {
        kind,
        availability,
        reason: (availability == RuntimeCapabilityAvailability::Unsupported).then(|| {
            "The current local evidence does not permit this operation safely.".to_owned()
        }),
    }
}

fn runtime_model_summary(model: MappedModel) -> RuntimeModelSummary {
    let mapping = if let Some(canonical_id) = model.canonical_id {
        RuntimeProviderModelMapping {
            status: RuntimeModelMappingStatus::Matched,
            catalogue_id: Some(CandidateModelId::new(canonical_id)),
        }
    } else {
        RuntimeProviderModelMapping {
            status: RuntimeModelMappingStatus::External,
            catalogue_id: None,
        }
    };
    RuntimeModelSummary {
        display_name: model.provider_id.clone(),
        provider_model_id: RuntimeProviderModelId::new(model.provider_id),
        size_bytes: Some(model.size_bytes),
        mapping,
    }
}

fn reason(code: RuntimeReasonCode, message: &str) -> RuntimeReason {
    RuntimeReason {
        code,
        message: message.to_owned(),
    }
}

fn warning(code: RuntimeWarningCode, message: &str) -> RuntimeWarning {
    RuntimeWarning {
        code,
        message: message.to_owned(),
    }
}

impl From<OllamaAdapterError> for RuntimeError {
    fn from(error: OllamaAdapterError) -> Self {
        match error {
            OllamaAdapterError::UnsafeEndpoint | OllamaAdapterError::InvalidEndpoint => {
                Self::EndpointUnsafe
            }
            OllamaAdapterError::ConnectionFailed | OllamaAdapterError::HttpStatus { .. } => {
                Self::ProviderUnavailable
            }
            OllamaAdapterError::ResponseTooLarge | OllamaAdapterError::ProcessOutputTooLarge => {
                Self::OutputLimit
            }
            OllamaAdapterError::ExecutableUntrusted => Self::ExecutableUntrusted,
            OllamaAdapterError::InvalidExecutable
            | OllamaAdapterError::ExecutableIo { .. }
            | OllamaAdapterError::InvalidResponse
            | OllamaAdapterError::InvalidVersion => Self::InvalidResponse,
            OllamaAdapterError::TimedOut => Self::TimedOut,
            OllamaAdapterError::Cancelled => Self::Cancelled,
            OllamaAdapterError::ModelAcquisitionFailed => Self::ModelAcquisitionFailed,
            OllamaAdapterError::StorageUnavailable => Self::ModelStorageUnavailable,
            OllamaAdapterError::ReadinessFailed => Self::ReadinessInferenceFailed,
            OllamaAdapterError::GenerationFailed => Self::GenerationFailed,
            OllamaAdapterError::LifecycleUnsupported => Self::Unsupported,
            OllamaAdapterError::LifecycleConflict => Self::Busy,
            OllamaAdapterError::ProcessStartFailed | OllamaAdapterError::ProcessControlFailed => {
                Self::ProcessFailed
            }
            OllamaAdapterError::Internal => Self::Internal,
        }
    }
}

const CHAT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const CHAT_MAX_LINE_BYTES: usize = 64 * 1024;
const CHAT_MAX_EVENTS: usize = 16 * 1024;
const CHAT_NUM_CTX: u32 = 4096;

const fn chat_limits(max_output_bytes: usize) -> ChatHttpLimits {
    ChatHttpLimits {
        connect_timeout: HTTP_TIMEOUT,
        idle_timeout: CHAT_IDLE_TIMEOUT,
        max_line_bytes: CHAT_MAX_LINE_BYTES,
        max_events: CHAT_MAX_EVENTS,
        max_output_bytes,
        num_ctx: CHAT_NUM_CTX,
    }
}

const fn chat_role(role: RuntimeChatRole) -> Option<&'static str> {
    match role {
        RuntimeChatRole::User => Some("user"),
        RuntimeChatRole::Assistant => Some("assistant"),
        _ => None,
    }
}

impl RuntimeChatProvider for OllamaAdapter {
    fn generate(
        &self,
        request: RuntimeChatRequest,
        deltas: ChatDeltaSender,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeGenerationResult> {
        let span = tracing::info_span!(
            "runtime_provider_chat_generate",
            provider_id = OLLAMA_PROVIDER_ID,
            correlation_id = %context.correlation_id(),
            request_id = %context.request_id(),
            context_messages = request.messages.len(),
        );
        Box::pin(
            async move {
                context.check()?;
                if request.messages.is_empty() || request.max_output_bytes == 0 {
                    return Err(RuntimeError::InvalidInput);
                }
                let provider_tag = provider_tag_for_candidate(&request.canonical_model_id)
                    .ok_or(RuntimeError::ModelNotMapped)?;
                let endpoint = self.valid_endpoint()?;
                let mut messages = Vec::with_capacity(request.messages.len());
                for message in &request.messages {
                    let role = chat_role(message.role).ok_or(RuntimeError::InvalidInput)?;
                    messages.push(ChatRequestMessage {
                        role,
                        content: message.content.as_str(),
                    });
                }

                let outcome = run_bounded(
                    &context,
                    self.chat_http.generate(
                        endpoint,
                        provider_tag,
                        &messages,
                        deltas,
                        chat_limits(request.max_output_bytes),
                    ),
                )
                .await
                .map_err(RuntimeError::from)?;

                Ok(RuntimeGenerationResult {
                    emitted_bytes: outcome.emitted_bytes,
                    completed_at_unix_ms: self.clock.unix_ms(),
                })
            }
            .instrument(span),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs, future,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use gixgiz_contracts::{
        CorrelationId, ModelAcquisitionPhase, ModelAcquisitionProgress, RequestId,
    };
    use gixgiz_runtime::RuntimeCancellationToken;
    use tempfile::tempdir;
    use tokio::sync::{Mutex, Notify, mpsc};

    use crate::{
        discovery::FixedExecutableLocator,
        http::{HttpFuture, tests::FakeHttpClient},
        model_http::{ModelHttpFuture, PullOutcome, tests::FakeModelHttpClient},
        process::ProcessFuture,
        storage::tests::FakeModelStorageProbe,
    };

    use super::*;

    const VERSION_RESPONSE: &[u8] = br#"{"version":"0.12.6"}"#;
    const EMPTY_TAGS_RESPONSE: &[u8] = br#"{"models":[]}"#;
    const INSTALLED_TAGS_RESPONSE: &[u8] = br#"{"models":[{"name":"qwen2.5:0.5b-instruct","model":"qwen2.5:0.5b-instruct","size":123,"digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}]}"#;

    fn context(timeout: Duration) -> RuntimeOperationContext {
        RuntimeOperationContext::new(CorrelationId::new(), RequestId::new(), timeout)
    }

    fn absent_locator() -> Arc<dyn ExecutableLocator> {
        FixedExecutableLocator::new(Ok(None))
    }

    fn executable_locator() -> (tempfile::TempDir, Arc<dyn ExecutableLocator>) {
        let directory = tempdir().unwrap();
        let path = directory.path().join("ollama.exe");
        fs::write(&path, b"fixture").unwrap();
        let locator = FixedExecutableLocator::from_path(&path);
        (directory, locator)
    }

    fn adapter(
        locator: Arc<dyn ExecutableLocator>,
        http: Arc<dyn OllamaHttpClient>,
        processes: Arc<dyn OwnedProcessControl>,
        policy: VersionPolicy,
    ) -> OllamaAdapter {
        OllamaAdapter::with_components(
            EndpointSelection::from_value(None),
            locator,
            http,
            processes,
            Arc::new(FixedClock(42)),
            policy,
        )
    }

    fn setup_adapter(
        responses: impl IntoIterator<Item = Result<Vec<u8>, OllamaAdapterError>>,
        model_http: Arc<dyn OllamaModelHttpClient>,
        storage: Result<u64, OllamaAdapterError>,
    ) -> OllamaAdapter {
        adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new(responses)),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            recorded_test_policy(),
        )
        .with_model_setup_components(model_http, Arc::new(FakeModelStorageProbe::new(storage)))
    }

    fn compact_plan(provider: &OllamaAdapter) -> RuntimeModelAcquisitionPlan {
        provider
            .prepare_model_acquisition_inner(
                CandidateModelId::new("qwen2.5.0.5b-instruct"),
                &context(Duration::from_secs(1)),
            )
            .expect("allowlisted compact plan")
    }

    fn recorded_test_policy() -> VersionPolicy {
        VersionPolicy::v0_1()
    }

    #[tokio::test]
    async fn no_executable_and_no_endpoint_is_not_installed() {
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            VersionPolicy::capability_first(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::NotInstalled);
        assert_eq!(
            observation.endpoint_safety,
            RuntimeEndpointSafety::LoopbackVerified
        );
    }

    #[tokio::test]
    async fn compatible_executable_with_unavailable_endpoint_is_installed_stopped() {
        let (_directory, locator) = executable_locator();
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            recorded_test_policy(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::InstalledStopped);
        assert!(observation.capabilities.iter().any(|capability| {
            capability.kind == RuntimeCapabilityKind::Start
                && capability.availability == RuntimeCapabilityAvailability::Available
        }));
    }

    #[tokio::test]
    async fn untrusted_executable_is_degraded_without_verified_or_start_claims() {
        let (_directory, locator) = executable_locator();
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            Arc::new(FakeProcesses::with_client_version_error(
                OllamaAdapterError::ExecutableUntrusted,
            )),
            recorded_test_policy(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Degraded);
        assert!(
            observation
                .reasons
                .iter()
                .any(|reason| reason.code == RuntimeReasonCode::EvidenceIncomplete)
        );
        assert!(
            observation
                .reasons
                .iter()
                .all(|reason| reason.code != RuntimeReasonCode::ExecutableVerified)
        );
        assert!(observation.capabilities.iter().any(|capability| {
            capability.kind == RuntimeCapabilityKind::Start
                && capability.availability == RuntimeCapabilityAvailability::Unsupported
        }));
    }

    #[tokio::test]
    async fn incomplete_executable_verification_never_claims_verified_or_startable() {
        for error in [
            OllamaAdapterError::InvalidExecutable,
            OllamaAdapterError::ExecutableIo {
                operation: "verify Authenticode",
                kind: std::io::ErrorKind::PermissionDenied,
            },
            OllamaAdapterError::ProcessOutputTooLarge,
        ] {
            let (_directory, locator) = executable_locator();
            let provider = adapter(
                locator,
                Arc::new(FakeHttpClient::new([Err(
                    OllamaAdapterError::ConnectionFailed,
                )])),
                Arc::new(FakeProcesses::with_client_version_error(error)),
                recorded_test_policy(),
            );

            let observation = provider
                .detect(context(Duration::from_secs(1)))
                .await
                .unwrap();

            assert_eq!(observation.state, RuntimeState::Degraded);
            assert!(
                observation
                    .reasons
                    .iter()
                    .any(|reason| reason.code == RuntimeReasonCode::EvidenceIncomplete)
            );
            assert!(
                observation
                    .reasons
                    .iter()
                    .all(|reason| reason.code != RuntimeReasonCode::ExecutableVerified)
            );
            assert!(observation.capabilities.iter().any(|capability| {
                capability.kind == RuntimeCapabilityKind::Start
                    && capability.availability == RuntimeCapabilityAvailability::Unsupported
            }));
        }
    }

    #[tokio::test]
    async fn incompatible_stopped_install_does_not_advertise_start() {
        let (_directory, locator) = executable_locator();
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            VersionPolicy::with_tested_range(
                semver::Version::new(0, 20, 0),
                semver::Version::new(0, 30, 0),
            )
            .unwrap(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Incompatible);
        assert!(observation.capabilities.iter().any(|capability| {
            capability.kind == RuntimeCapabilityKind::Start
                && capability.availability == RuntimeCapabilityAvailability::Unsupported
        }));
    }

    #[tokio::test]
    async fn unverifiable_owned_process_state_is_degraded_without_exit_claim() {
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([])),
            Arc::new(FakeProcesses::failing_status()),
            VersionPolicy::capability_first(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Degraded);
        assert_eq!(
            observation.reasons[0].code,
            RuntimeReasonCode::EvidenceIncomplete
        );
        assert_eq!(
            observation.warnings[0].code,
            RuntimeWarningCode::PartialEvidence
        );
    }

    #[tokio::test]
    async fn cancellation_during_owned_process_detection_is_propagated() {
        let cancellation = RuntimeCancellationToken::new();
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([])),
            Arc::new(FakeProcesses::cancelling_status(cancellation.clone())),
            VersionPolicy::capability_first(),
        );

        let result = provider
            .detect(context(Duration::from_secs(1)).with_cancellation(cancellation))
            .await;

        assert_eq!(result, Err(RuntimeError::Cancelled));
    }

    #[tokio::test]
    async fn exited_owned_child_with_unavailable_endpoint_is_failed() {
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::Exited)),
            VersionPolicy::capability_first(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Failed);
        assert_eq!(
            observation.reasons[0].code,
            RuntimeReasonCode::ProcessExited
        );
    }

    #[tokio::test]
    async fn running_owned_child_without_health_is_degraded_outside_start() {
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::Running)),
            VersionPolicy::capability_first(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Degraded);
        assert!(observation.capabilities.iter().any(|capability| {
            capability.kind == RuntimeCapabilityKind::Stop
                && capability.availability == RuntimeCapabilityAvailability::Available
        }));
    }

    #[tokio::test]
    async fn compatible_required_endpoints_are_ready() {
        let (_directory, locator) = executable_locator();
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
            ])),
            Arc::new(FakeProcesses::with_client_version_error(
                OllamaAdapterError::ExecutableUntrusted,
            )),
            recorded_test_policy(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Ready);
        assert_eq!(
            observation.version.unwrap().compatibility,
            RuntimeVersionCompatibility::Compatible
        );
        assert!(observation.capabilities.iter().any(|capability| {
            capability.kind == RuntimeCapabilityKind::Start
                && capability.availability == RuntimeCapabilityAvailability::Unsupported
        }));
        assert!(
            observation
                .reasons
                .iter()
                .all(|reason| reason.code != RuntimeReasonCode::ExecutableVerified)
        );
    }

    #[tokio::test]
    async fn future_server_version_is_degraded_without_compatible_reason() {
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([
                Ok(br#"{"version":"0.32.6"}"#.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
            ])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            VersionPolicy::v0_1(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Degraded);
        assert_eq!(
            observation.version.as_ref().unwrap().compatibility,
            RuntimeVersionCompatibility::Untested
        );
        assert!(
            observation
                .reasons
                .iter()
                .any(|reason| reason.code == RuntimeReasonCode::VersionUnverified)
        );
        assert!(
            observation
                .reasons
                .iter()
                .all(|reason| reason.code != RuntimeReasonCode::VersionCompatible)
        );
    }

    #[tokio::test]
    async fn production_below_range_version_is_incompatible() {
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([Ok(
                br#"{"version":"0.12.5"}"#.to_vec()
            )])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            VersionPolicy::v0_1(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Incompatible);
        assert_eq!(
            observation.version.as_ref().unwrap().compatibility,
            RuntimeVersionCompatibility::Incompatible
        );
    }

    #[tokio::test]
    async fn malformed_reachable_provider_is_degraded() {
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([Ok(b"not-json".to_vec())])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            VersionPolicy::capability_first(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Degraded);
    }

    #[tokio::test]
    async fn inaccessible_executable_evidence_is_degraded_without_raw_path_details() {
        let provider = adapter(
            FixedExecutableLocator::new(Err(OllamaAdapterError::ExecutableIo {
                operation: "metadata",
                kind: std::io::ErrorKind::PermissionDenied,
            })),
            Arc::new(FakeHttpClient::new([])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            VersionPolicy::capability_first(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Degraded);
        assert!(
            observation
                .reasons
                .iter()
                .all(|reason| !reason.message.contains(':'))
        );
    }

    #[tokio::test]
    async fn unsafe_endpoint_is_reported_without_network_io() {
        let provider = OllamaAdapter::with_components(
            EndpointSelection::from_value(Some("0.0.0.0:11434")),
            absent_locator(),
            Arc::new(FakeHttpClient::new([])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            Arc::new(FixedClock(42)),
            VersionPolicy::capability_first(),
        );

        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Degraded);
        assert_eq!(observation.endpoint_safety, RuntimeEndpointSafety::Unsafe);
    }

    #[tokio::test]
    async fn model_inventory_maps_known_and_preserves_unknown_identifiers() {
        let tags = br#"{"models":[{"name":"qwen2.5:0.5b-instruct","model":"qwen2.5:0.5b-instruct","size":123,"digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},{"name":"private:latest","model":"private:latest","size":456,"digest":"sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"}]}"#;
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(tags.to_vec()),
            ])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            VersionPolicy::capability_first(),
        );

        let inventory = provider
            .list_models(10, context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(inventory.collected_at_unix_ms, 42);
        assert_eq!(inventory.models.len(), 2);
        assert_eq!(
            inventory.models[0].mapping.status,
            RuntimeModelMappingStatus::Matched
        );
        assert_eq!(
            inventory.models[0]
                .mapping
                .catalogue_id
                .as_ref()
                .unwrap()
                .as_str(),
            "qwen2.5.0.5b-instruct"
        );
        assert_eq!(
            inventory.models[1].mapping.status,
            RuntimeModelMappingStatus::External
        );
    }

    #[tokio::test]
    async fn missing_required_model_route_is_incompatible_for_inventory() {
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([
                Ok(VERSION_RESPONSE.to_vec()),
                Err(OllamaAdapterError::HttpStatus { status: 404 }),
            ])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            VersionPolicy::capability_first(),
        );

        let result = provider
            .list_models(10, context(Duration::from_secs(1)))
            .await;

        assert_eq!(result, Err(RuntimeError::IncompatibleVersion));
    }

    #[tokio::test]
    async fn missing_required_version_route_is_incompatible_for_start() {
        let processes = Arc::new(FakeProcesses::new(OwnedProcessStatus::None));
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([Err(OllamaAdapterError::HttpStatus {
                status: 404,
            })])),
            processes.clone(),
            VersionPolicy::capability_first(),
        );

        let result = provider
            .execute(RuntimeOperationKind::Start, context(Duration::from_secs(1)))
            .await;

        assert_eq!(result, Err(RuntimeError::IncompatibleVersion));
        assert_eq!(processes.starts.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn untrusted_executable_blocks_start_before_provider_process_execution() {
        let (_directory, locator) = executable_locator();
        let processes = Arc::new(FakeProcesses::with_client_version_error(
            OllamaAdapterError::ExecutableUntrusted,
        ));
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            processes.clone(),
            recorded_test_policy(),
        );

        let result = provider
            .execute(RuntimeOperationKind::Start, context(Duration::from_secs(1)))
            .await;

        assert_eq!(result, Err(RuntimeError::ExecutableUntrusted));
        assert_eq!(processes.starts.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn missing_required_model_route_after_spawn_rolls_back_as_incompatible() {
        let (_directory, locator) = executable_locator();
        let processes = Arc::new(FakeProcesses::new(OwnedProcessStatus::None));
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([
                Err(OllamaAdapterError::ConnectionFailed),
                Ok(VERSION_RESPONSE.to_vec()),
                Err(OllamaAdapterError::HttpStatus { status: 404 }),
            ])),
            processes.clone(),
            recorded_test_policy(),
        );

        let result = provider
            .execute(RuntimeOperationKind::Start, context(Duration::from_secs(1)))
            .await;

        assert_eq!(result, Err(RuntimeError::IncompatibleVersion));
        assert_eq!(processes.starts.load(Ordering::Acquire), 1);
        assert_eq!(processes.stops.load(Ordering::Acquire), 1);
    }

    #[tokio::test]
    async fn stop_refuses_any_process_not_owned_by_this_adapter() {
        let provider = adapter(
            absent_locator(),
            Arc::new(FakeHttpClient::new([])),
            Arc::new(FakeProcesses::new(OwnedProcessStatus::None)),
            VersionPolicy::capability_first(),
        );

        let result = provider
            .execute(RuntimeOperationKind::Stop, context(Duration::from_secs(1)))
            .await;

        assert_eq!(result, Err(RuntimeError::OwnershipConflict));
    }

    #[tokio::test]
    async fn stop_commit_ignores_late_cancellation_and_returns_stopped_observation() {
        let (_directory, locator) = executable_locator();
        let cancellation = RuntimeCancellationToken::new();
        let processes = Arc::new(FakeProcesses::cancelling_on_stop(cancellation.clone()));
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            processes.clone(),
            recorded_test_policy(),
        );

        let observation = provider
            .execute(
                RuntimeOperationKind::Stop,
                context(Duration::from_secs(1)).with_cancellation(cancellation.clone()),
            )
            .await
            .unwrap();

        assert!(cancellation.is_cancelled());
        assert_eq!(observation.state, RuntimeState::InstalledStopped);
        assert_eq!(processes.stops.load(Ordering::Acquire), 1);
    }

    #[tokio::test]
    async fn restart_preserves_cancellation_after_committed_stop() {
        let (_directory, locator) = executable_locator();
        let cancellation = RuntimeCancellationToken::new();
        let processes = Arc::new(FakeProcesses::cancelling_on_stop(cancellation.clone()));
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([])),
            processes.clone(),
            VersionPolicy::capability_first(),
        );

        let result = provider
            .execute(
                RuntimeOperationKind::Restart,
                context(Duration::from_secs(1)).with_cancellation(cancellation),
            )
            .await;

        assert_eq!(result, Err(RuntimeError::Cancelled));
        assert_eq!(processes.stops.load(Ordering::Acquire), 1);
        assert_eq!(processes.starts.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn managed_start_is_verified_and_exposes_owned_stop_only() {
        let (_directory, locator) = executable_locator();
        let processes = Arc::new(FakeProcesses::new(OwnedProcessStatus::None));
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([
                Err(OllamaAdapterError::ConnectionFailed),
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
            ])),
            processes.clone(),
            recorded_test_policy(),
        );

        let observation = provider
            .execute(RuntimeOperationKind::Start, context(Duration::from_secs(1)))
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Ready);
        assert_eq!(processes.starts.load(Ordering::Acquire), 1);
        assert!(observation.capabilities.iter().any(|capability| {
            capability.kind == RuntimeCapabilityKind::Stop
                && capability.availability == RuntimeCapabilityAvailability::Available
        }));
    }

    #[tokio::test]
    async fn concurrent_detection_reports_starting_for_owned_start_phase() {
        let (_directory, locator) = executable_locator();
        let cancellation = RuntimeCancellationToken::new();
        let processes = Arc::new(FakeProcesses::new(OwnedProcessStatus::None));
        let http = Arc::new(PendingAfterFirstHttp::default());
        let provider = Arc::new(adapter(
            locator,
            http.clone(),
            processes,
            recorded_test_policy(),
        ));
        let start_provider = Arc::clone(&provider);
        let start_cancellation = cancellation.clone();
        let start_task = tokio::spawn(async move {
            start_provider
                .execute(
                    RuntimeOperationKind::Start,
                    context(Duration::from_secs(1)).with_cancellation(start_cancellation),
                )
                .await
        });

        http.pending_started.notified().await;
        let observation = provider
            .detect(context(Duration::from_secs(1)))
            .await
            .unwrap();
        cancellation.cancel();

        assert_eq!(observation.state, RuntimeState::Starting);
        assert_eq!(start_task.await.unwrap(), Err(RuntimeError::Cancelled));
    }

    #[tokio::test]
    async fn startup_timeout_rolls_back_exact_owned_child() {
        let (_directory, locator) = executable_locator();
        let processes = Arc::new(FakeProcesses::new(OwnedProcessStatus::None));
        let http = Arc::new(PendingAfterFirstHttp::default());
        let provider = Arc::new(adapter(
            locator,
            http.clone(),
            processes.clone(),
            recorded_test_policy(),
        ));
        let task = tokio::spawn(async move {
            provider
                .execute(
                    RuntimeOperationKind::Start,
                    context(Duration::from_millis(100)),
                )
                .await
        });

        http.pending_started.notified().await;
        let result = task.await.unwrap();

        assert_eq!(result, Err(RuntimeError::TimedOut));
        assert_eq!(processes.stops.load(Ordering::Acquire), 1);
        assert_eq!(
            processes.owned_status().await.unwrap(),
            OwnedProcessStatus::None
        );
    }

    #[tokio::test]
    async fn startup_cancellation_rolls_back_exact_owned_child() {
        let (_directory, locator) = executable_locator();
        let processes = Arc::new(FakeProcesses::new(OwnedProcessStatus::None));
        let http = Arc::new(PendingAfterFirstHttp::default());
        let cancellation = RuntimeCancellationToken::new();
        let operation_context =
            context(Duration::from_secs(1)).with_cancellation(cancellation.clone());
        let provider = Arc::new(adapter(
            locator,
            http.clone(),
            processes.clone(),
            recorded_test_policy(),
        ));
        let task = tokio::spawn(async move {
            provider
                .execute(RuntimeOperationKind::Start, operation_context)
                .await
        });

        http.pending_started.notified().await;
        cancellation.cancel();
        let result = task.await.unwrap();

        assert_eq!(result, Err(RuntimeError::Cancelled));
        assert_eq!(processes.stops.load(Ordering::Acquire), 1);
        assert_eq!(
            processes.owned_status().await.unwrap(),
            OwnedProcessStatus::None
        );
    }

    #[tokio::test]
    async fn cancellation_immediately_after_spawn_rolls_back_exact_owned_child() {
        let (_directory, locator) = executable_locator();
        let cancellation = RuntimeCancellationToken::new();
        let processes = Arc::new(FakeProcesses::cancelling_on_start(cancellation.clone()));
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            processes.clone(),
            recorded_test_policy(),
        );

        let result = provider
            .execute(
                RuntimeOperationKind::Start,
                context(Duration::from_secs(1)).with_cancellation(cancellation),
            )
            .await;

        assert_eq!(result, Err(RuntimeError::Cancelled));
        assert_eq!(processes.stops.load(Ordering::Acquire), 1);
        assert_eq!(
            processes.owned_status().await.unwrap(),
            OwnedProcessStatus::None
        );
    }

    #[tokio::test]
    async fn competing_start_conflict_does_not_stop_another_owned_child() {
        let (_directory, locator) = executable_locator();
        let processes = Arc::new(FakeProcesses::conflicting_start());
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            processes.clone(),
            recorded_test_policy(),
        );

        let result = provider
            .execute(RuntimeOperationKind::Start, context(Duration::from_secs(1)))
            .await;

        assert_eq!(result, Err(RuntimeError::Busy));
        assert_eq!(processes.stops.load(Ordering::Acquire), 0);
        assert_eq!(
            processes.owned_status().await.unwrap(),
            OwnedProcessStatus::Running
        );
    }

    #[tokio::test]
    async fn child_exit_during_start_is_a_safe_process_failure() {
        let (_directory, locator) = executable_locator();
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([Err(
                OllamaAdapterError::ConnectionFailed,
            )])),
            Arc::new(FakeProcesses::exiting_after_start()),
            recorded_test_policy(),
        );

        let result = provider
            .execute(RuntimeOperationKind::Start, context(Duration::from_secs(1)))
            .await;

        assert_eq!(result, Err(RuntimeError::ProcessFailed));
    }

    #[tokio::test]
    async fn restart_is_one_owned_stop_followed_by_one_verified_start() {
        let (_directory, locator) = executable_locator();
        let processes = Arc::new(FakeProcesses::new(OwnedProcessStatus::Running));
        let provider = adapter(
            locator,
            Arc::new(FakeHttpClient::new([
                Err(OllamaAdapterError::ConnectionFailed),
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
            ])),
            processes.clone(),
            recorded_test_policy(),
        );

        let observation = provider
            .execute(
                RuntimeOperationKind::Restart,
                context(Duration::from_secs(1)),
            )
            .await
            .unwrap();

        assert_eq!(observation.state, RuntimeState::Ready);
        assert_eq!(processes.stops.load(Ordering::Acquire), 1);
        assert_eq!(processes.starts.load(Ordering::Acquire), 1);
    }

    #[tokio::test]
    async fn setup_preparation_is_allowlisted_and_never_returns_a_private_path() {
        let provider = setup_adapter(
            [],
            Arc::new(FakeModelHttpClient::new([], [], Vec::new())),
            Ok(10_000),
        );

        let plan = provider
            .prepare_model_acquisition(
                CandidateModelId::new("qwen2.5.0.5b-instruct"),
                context(Duration::from_secs(1)),
            )
            .await
            .expect("allowlisted model maps");
        let unknown = provider
            .prepare_model_acquisition(
                CandidateModelId::new("private.unreviewed"),
                context(Duration::from_secs(1)),
            )
            .await;

        assert_eq!(
            plan.artifact.provider_model_id.as_str(),
            "qwen2.5:0.5b-instruct"
        );
        assert_eq!(plan.destination, SetupDestinationCategory::ProviderManaged);
        assert!(!plan.destination_display.contains('\\'));
        assert!(!plan.destination_display.contains('/'));
        assert_eq!(unknown, Err(RuntimeError::ModelNotMapped));

        let mut tampered = plan;
        tampered.artifact.source_summary = "caller supplied source".to_owned();
        let tampered_result = provider
            .preflight_model_storage(tampered, 1, 1, context(Duration::from_secs(1)))
            .await;
        assert_eq!(tampered_result, Err(RuntimeError::InvalidInput));
    }

    #[tokio::test]
    async fn storage_preflight_reports_available_insufficient_and_unavailable() {
        let sufficient = setup_adapter(
            [],
            Arc::new(FakeModelHttpClient::new([], [], Vec::new())),
            Ok(3_000),
        );
        let available = sufficient
            .preflight_model_storage(
                compact_plan(&sufficient),
                1_000,
                2_000,
                context(Duration::from_secs(1)),
            )
            .await
            .expect("storage is checked");
        let insufficient = setup_adapter(
            [],
            Arc::new(FakeModelHttpClient::new([], [], Vec::new())),
            Ok(2_999),
        );
        let insufficient_result = insufficient
            .preflight_model_storage(
                compact_plan(&insufficient),
                1_000,
                2_000,
                context(Duration::from_secs(1)),
            )
            .await
            .expect("insufficient space is a typed result");
        let unavailable = setup_adapter(
            [],
            Arc::new(FakeModelHttpClient::new([], [], Vec::new())),
            Err(OllamaAdapterError::StorageUnavailable),
        );
        let unavailable_result = unavailable
            .preflight_model_storage(
                compact_plan(&unavailable),
                1_000,
                2_000,
                context(Duration::from_secs(1)),
            )
            .await
            .expect("unavailable destination is a typed result");

        assert_eq!(
            available.availability,
            RuntimeStorageAvailability::Available
        );
        assert_eq!(available.available_bytes, Some(3_000));
        assert_eq!(
            insufficient_result.availability,
            RuntimeStorageAvailability::InsufficientSpace
        );
        assert_eq!(
            unavailable_result.availability,
            RuntimeStorageAvailability::Unavailable
        );
        assert_eq!(unavailable_result.available_bytes, None);
    }

    #[tokio::test]
    async fn existing_exact_model_is_reused_without_a_pull() {
        let provider = setup_adapter(
            [
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(INSTALLED_TAGS_RESPONSE.to_vec()),
            ],
            Arc::new(FakeModelHttpClient::new([], [], Vec::new())),
            Ok(10_000),
        );
        let (sender, _receiver) = mpsc::channel(1);

        let result = provider
            .acquire_model(
                compact_plan(&provider),
                sender,
                context(Duration::from_secs(1)),
            )
            .await
            .expect("existing exact model is reused");

        assert_eq!(result.status, RuntimeModelAcquisitionStatus::AlreadyPresent);
        assert_eq!(result.measured_size_bytes, Some(123));
        assert_eq!(result.integrity, ModelIntegrityState::ProviderReported);
    }

    #[tokio::test]
    async fn successful_pull_emits_normalized_progress_and_requires_registration() {
        let progress = ModelAcquisitionProgress {
            phase: ModelAcquisitionPhase::Transferring,
            completed_bytes: Some(50),
            total_bytes: Some(100),
            progress_basis_points: Some(5_000),
        };
        let provider = setup_adapter(
            [
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(INSTALLED_TAGS_RESPONSE.to_vec()),
            ],
            Arc::new(FakeModelHttpClient::new(
                [Ok(PullOutcome {
                    provider_integrity: true,
                })],
                [],
                vec![progress.clone()],
            )),
            Ok(10_000),
        );
        let (sender, mut receiver) = mpsc::channel(4);

        let result = provider
            .acquire_model(
                compact_plan(&provider),
                sender,
                context(Duration::from_secs(1)),
            )
            .await
            .expect("pull and exact registration succeed");

        assert_eq!(result.status, RuntimeModelAcquisitionStatus::Acquired);
        assert_eq!(receiver.try_recv(), Ok(progress));
    }

    #[tokio::test]
    async fn successful_pull_without_exact_registration_fails_closed() {
        let provider = setup_adapter(
            [
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
            ],
            Arc::new(FakeModelHttpClient::new(
                [Ok(PullOutcome {
                    provider_integrity: true,
                })],
                [],
                Vec::new(),
            )),
            Ok(10_000),
        );
        let (sender, _receiver) = mpsc::channel(1);

        let result = provider
            .acquire_model(
                compact_plan(&provider),
                sender,
                context(Duration::from_secs(1)),
            )
            .await;

        assert_eq!(result, Err(RuntimeError::ModelRegistrationFailed));
    }

    #[tokio::test]
    async fn readiness_failure_is_typed_and_generated_content_never_returns() {
        let provider = setup_adapter(
            [
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(INSTALLED_TAGS_RESPONSE.to_vec()),
            ],
            Arc::new(FakeModelHttpClient::new(
                [],
                [Err(OllamaAdapterError::ReadinessFailed)],
                Vec::new(),
            )),
            Ok(10_000),
        );
        let artifact = compact_plan(&provider).artifact;

        let result = provider
            .run_readiness_inference(artifact, context(Duration::from_secs(1)))
            .await;

        assert_eq!(result, Err(RuntimeError::ReadinessInferenceFailed));
    }

    #[tokio::test]
    async fn cancellation_aborts_the_active_pull_and_releases_the_guard() {
        let model_http = Arc::new(PendingModelHttpClient::default());
        let provider = setup_adapter(
            [
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
                Ok(VERSION_RESPONSE.to_vec()),
                Ok(EMPTY_TAGS_RESPONSE.to_vec()),
            ],
            model_http.clone(),
            Ok(10_000),
        );
        let cancellation = RuntimeCancellationToken::new();
        let operation_context =
            context(Duration::from_secs(5)).with_cancellation(cancellation.clone());
        let (sender, _receiver) = mpsc::channel(1);
        let operation = provider.acquire_model(compact_plan(&provider), sender, operation_context);
        tokio::pin!(operation);

        tokio::select! {
            () = model_http.pull_started.notified() => {}
            result = &mut operation => panic!("pull completed before cancellation: {result:?}"),
        }
        cancellation.cancel();

        assert_eq!(operation.await, Err(RuntimeError::Cancelled));
        assert!(!provider.acquiring.load(Ordering::Acquire));
    }

    #[derive(Default)]
    struct PendingModelHttpClient {
        pull_started: Notify,
    }

    impl OllamaModelHttpClient for PendingModelHttpClient {
        fn pull<'a>(
            &'a self,
            _endpoint: &'a ValidatedEndpoint,
            _provider_model_id: &'a str,
            _progress: ModelProgressSender,
            _limits: ModelHttpLimits,
        ) -> ModelHttpFuture<'a, PullOutcome> {
            Box::pin(async move {
                self.pull_started.notify_one();
                future::pending().await
            })
        }

        fn readiness<'a>(
            &'a self,
            _endpoint: &'a ValidatedEndpoint,
            _provider_model_id: &'a str,
            _limits: ModelHttpLimits,
        ) -> ModelHttpFuture<'a, ()> {
            Box::pin(future::pending())
        }
    }

    struct FixedClock(u64);

    impl Clock for FixedClock {
        fn unix_ms(&self) -> u64 {
            self.0
        }
    }

    struct FakeProcesses {
        status: Mutex<OwnedProcessStatus>,
        starts: AtomicUsize,
        stops: AtomicUsize,
        client_version_error: Option<OllamaAdapterError>,
        exit_on_start: bool,
        conflict_on_start: bool,
        cancel_on_start: Option<RuntimeCancellationToken>,
        cancel_on_stop: Option<RuntimeCancellationToken>,
        cancel_during_status: Option<RuntimeCancellationToken>,
        status_error: bool,
    }

    impl FakeProcesses {
        fn new(status: OwnedProcessStatus) -> Self {
            Self {
                status: Mutex::new(status),
                starts: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                client_version_error: None,
                exit_on_start: false,
                conflict_on_start: false,
                cancel_on_start: None,
                cancel_on_stop: None,
                cancel_during_status: None,
                status_error: false,
            }
        }

        fn with_client_version_error(error: OllamaAdapterError) -> Self {
            Self {
                client_version_error: Some(error),
                ..Self::new(OwnedProcessStatus::None)
            }
        }

        fn exiting_after_start() -> Self {
            Self {
                status: Mutex::new(OwnedProcessStatus::None),
                starts: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                client_version_error: None,
                exit_on_start: true,
                conflict_on_start: false,
                cancel_on_start: None,
                cancel_on_stop: None,
                cancel_during_status: None,
                status_error: false,
            }
        }

        fn cancelling_on_start(cancellation: RuntimeCancellationToken) -> Self {
            Self {
                status: Mutex::new(OwnedProcessStatus::None),
                starts: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                client_version_error: None,
                exit_on_start: false,
                conflict_on_start: false,
                cancel_on_start: Some(cancellation),
                cancel_on_stop: None,
                cancel_during_status: None,
                status_error: false,
            }
        }

        fn failing_status() -> Self {
            Self {
                status: Mutex::new(OwnedProcessStatus::None),
                starts: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                client_version_error: None,
                exit_on_start: false,
                conflict_on_start: false,
                cancel_on_start: None,
                cancel_on_stop: None,
                cancel_during_status: None,
                status_error: true,
            }
        }

        fn cancelling_status(cancellation: RuntimeCancellationToken) -> Self {
            Self {
                status: Mutex::new(OwnedProcessStatus::None),
                starts: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                client_version_error: None,
                exit_on_start: false,
                conflict_on_start: false,
                cancel_on_start: None,
                cancel_on_stop: None,
                cancel_during_status: Some(cancellation),
                status_error: false,
            }
        }

        fn cancelling_on_stop(cancellation: RuntimeCancellationToken) -> Self {
            Self {
                status: Mutex::new(OwnedProcessStatus::Running),
                starts: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                client_version_error: None,
                exit_on_start: false,
                conflict_on_start: false,
                cancel_on_start: None,
                cancel_on_stop: Some(cancellation),
                cancel_during_status: None,
                status_error: false,
            }
        }

        fn conflicting_start() -> Self {
            Self {
                status: Mutex::new(OwnedProcessStatus::None),
                starts: AtomicUsize::new(0),
                stops: AtomicUsize::new(0),
                client_version_error: None,
                exit_on_start: false,
                conflict_on_start: true,
                cancel_on_start: None,
                cancel_on_stop: None,
                cancel_during_status: None,
                status_error: false,
            }
        }
    }

    impl OwnedProcessControl for FakeProcesses {
        fn client_version<'a>(
            &'a self,
            _executable: &'a ValidatedExecutable,
            _endpoint: &'a ValidatedEndpoint,
            _limits: ProcessLimits,
        ) -> ProcessFuture<'a, semver::Version> {
            let error = self.client_version_error.clone();
            Box::pin(async move {
                match error {
                    Some(error) => Err(error),
                    None => Ok(semver::Version::new(0, 12, 6)),
                }
            })
        }

        fn start_owned<'a>(
            &'a self,
            _executable: &'a ValidatedExecutable,
            _endpoint: &'a ValidatedEndpoint,
        ) -> ProcessFuture<'a, u32> {
            Box::pin(async move {
                self.starts.fetch_add(1, Ordering::AcqRel);
                if self.conflict_on_start {
                    *self.status.lock().await = OwnedProcessStatus::Running;
                    return Err(OllamaAdapterError::LifecycleConflict);
                }
                *self.status.lock().await = if self.exit_on_start {
                    OwnedProcessStatus::Exited
                } else {
                    OwnedProcessStatus::Running
                };
                if let Some(cancellation) = &self.cancel_on_start {
                    cancellation.cancel();
                }
                Ok(42)
            })
        }

        fn stop_owned<'a>(&'a self, _timeout: Duration) -> ProcessFuture<'a, ()> {
            Box::pin(async move {
                let mut status = self.status.lock().await;
                if *status != OwnedProcessStatus::Running {
                    return Err(OllamaAdapterError::LifecycleUnsupported);
                }
                self.stops.fetch_add(1, Ordering::AcqRel);
                *status = OwnedProcessStatus::None;
                if let Some(cancellation) = &self.cancel_on_stop {
                    cancellation.cancel();
                }
                Ok(())
            })
        }

        fn owned_status<'a>(&'a self) -> ProcessFuture<'a, OwnedProcessStatus> {
            Box::pin(async move {
                if self.status_error {
                    return Err(OllamaAdapterError::ProcessControlFailed);
                }
                if let Some(cancellation) = &self.cancel_during_status {
                    cancellation.cancel();
                    future::pending().await
                }
                Ok(*self.status.lock().await)
            })
        }
    }

    #[derive(Default)]
    struct PendingAfterFirstHttp {
        calls: AtomicUsize,
        pending_started: tokio::sync::Notify,
    }

    impl OllamaHttpClient for PendingAfterFirstHttp {
        fn get<'a>(
            &'a self,
            _endpoint: &'a ValidatedEndpoint,
            _route: OllamaRoute,
            _limits: HttpLimits,
        ) -> HttpFuture<'a> {
            let call = self.calls.fetch_add(1, Ordering::AcqRel);
            match call {
                0 => Box::pin(async { Err(OllamaAdapterError::ConnectionFailed) }),
                1 => Box::pin(async move {
                    self.pending_started.notify_one();
                    future::pending().await
                }),
                _ => Box::pin(async { Err(OllamaAdapterError::ConnectionFailed) }),
            }
        }
    }
}
