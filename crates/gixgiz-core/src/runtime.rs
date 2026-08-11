//! Core-owned runtime policy and provider orchestration.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use gixgiz_contracts::{
    RUNTIME_REPORT_SCHEMA_VERSION, RuntimeCapabilityAvailability, RuntimeCapabilityDescriptor,
    RuntimeCapabilityKind, RuntimeConsentDecision, RuntimeConsentState, RuntimeEndpointSafety,
    RuntimeHealthReport, RuntimeModelInventory, RuntimeOperationKind, RuntimeOwnership,
    RuntimeProviderId, RuntimeState, RuntimeWarning, RuntimeWarningCode,
};
use gixgiz_persistence::{Persistence, RuntimePolicyRecord, RuntimePolicyRepository};
use gixgiz_runtime::{RuntimeError, RuntimeObservation, RuntimeOperationContext, RuntimeProvider};

const MAX_MODEL_INVENTORY_ITEMS: u16 = 100;

trait RuntimePolicyStore: Send + Sync {
    fn get(
        &self,
        provider_id: &RuntimeProviderId,
    ) -> Result<Option<RuntimePolicyRecord>, RuntimeError>;
    fn upsert(&self, policy: &RuntimePolicyRecord) -> Result<(), RuntimeError>;
}

#[derive(Clone)]
struct PersistenceRuntimePolicyStore {
    repository: RuntimePolicyRepository,
}

impl RuntimePolicyStore for PersistenceRuntimePolicyStore {
    fn get(
        &self,
        provider_id: &RuntimeProviderId,
    ) -> Result<Option<RuntimePolicyRecord>, RuntimeError> {
        self.repository
            .get(provider_id)
            .map_err(|_| RuntimeError::PolicyUnavailable)
    }

    fn upsert(&self, policy: &RuntimePolicyRecord) -> Result<(), RuntimeError> {
        self.repository
            .upsert(policy)
            .map_err(|_| RuntimeError::PolicyUnavailable)
    }
}

#[derive(Default)]
struct MemoryRuntimePolicyStore {
    records: Mutex<HashMap<RuntimeProviderId, RuntimePolicyRecord>>,
}

impl RuntimePolicyStore for MemoryRuntimePolicyStore {
    fn get(
        &self,
        provider_id: &RuntimeProviderId,
    ) -> Result<Option<RuntimePolicyRecord>, RuntimeError> {
        self.records
            .lock()
            .map_err(|_| RuntimeError::PolicyUnavailable)
            .map(|records| records.get(provider_id).cloned())
    }

    fn upsert(&self, policy: &RuntimePolicyRecord) -> Result<(), RuntimeError> {
        self.records
            .lock()
            .map_err(|_| RuntimeError::PolicyUnavailable)?
            .insert(policy.provider_id.clone(), policy.clone());
        Ok(())
    }
}

struct UnavailableRuntimePolicyStore;

impl RuntimePolicyStore for UnavailableRuntimePolicyStore {
    fn get(
        &self,
        _provider_id: &RuntimeProviderId,
    ) -> Result<Option<RuntimePolicyRecord>, RuntimeError> {
        Err(RuntimeError::PolicyUnavailable)
    }

    fn upsert(&self, _policy: &RuntimePolicyRecord) -> Result<(), RuntimeError> {
        Err(RuntimeError::PolicyUnavailable)
    }
}

/// Core-owned runtime orchestration with durable ownership and consent policy.
#[derive(Clone)]
pub struct RuntimeService {
    provider: Arc<dyn RuntimeProvider>,
    policy: Arc<dyn RuntimePolicyStore>,
    lifecycle: Arc<tokio::sync::Mutex<()>>,
}

impl RuntimeService {
    /// Creates a runtime service backed by Rust-owned persistence.
    #[must_use]
    pub fn with_persistence(provider: Arc<dyn RuntimeProvider>, persistence: &Persistence) -> Self {
        Self::new(
            provider,
            Arc::new(PersistenceRuntimePolicyStore {
                repository: persistence.runtime_policy(),
            }),
        )
    }

    /// Creates an in-memory policy service for deterministic tests only.
    #[must_use]
    pub fn in_memory(provider: Arc<dyn RuntimeProvider>) -> Self {
        Self::new(provider, Arc::new(MemoryRuntimePolicyStore::default()))
    }

    pub(crate) fn policy_unavailable(provider: Arc<dyn RuntimeProvider>) -> Self {
        Self::new(provider, Arc::new(UnavailableRuntimePolicyStore))
    }

    fn new(provider: Arc<dyn RuntimeProvider>, policy: Arc<dyn RuntimePolicyStore>) -> Self {
        Self {
            provider,
            policy,
            lifecycle: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Returns the provider identity handled by this service.
    #[must_use]
    pub fn provider_id(&self) -> RuntimeProviderId {
        self.provider.provider_id().clone()
    }

    /// Detects current provider evidence and composes authoritative policy.
    pub async fn status(
        &self,
        provider_id: &RuntimeProviderId,
        context: RuntimeOperationContext,
    ) -> Result<RuntimeHealthReport, RuntimeError> {
        self.ensure_provider(provider_id)?;
        let observation = self.provider.detect(context.clone()).await?;
        let policy = self.load_policy(provider_id, &context).await?;
        Ok(compose_report(observation, policy.as_ref()))
    }

    /// Records an explicit reuse decision without changing runtime ownership.
    pub async fn set_reuse_consent(
        &self,
        provider_id: &RuntimeProviderId,
        decision: RuntimeConsentDecision,
        context: RuntimeOperationContext,
    ) -> Result<RuntimeHealthReport, RuntimeError> {
        self.ensure_provider(provider_id)?;
        let reuse_consent = match decision {
            RuntimeConsentDecision::ApproveReuse => RuntimeConsentState::ReuseApproved,
            RuntimeConsentDecision::DenyReuse => RuntimeConsentState::Denied,
            RuntimeConsentDecision::Unknown => return Err(RuntimeError::InvalidInput),
            _ => return Err(RuntimeError::InvalidInput),
        };
        let observation = self.provider.detect(context.clone()).await?;
        if observation.state == RuntimeState::NotInstalled {
            return Err(RuntimeError::NotInstalled);
        }
        let current = self.load_policy(provider_id, &context).await?;
        ensure_reuse_decision(&observation, current.as_ref(), decision)?;
        let policy = RuntimePolicyRecord::new(
            provider_id.clone(),
            current
                .as_ref()
                .map_or(RuntimeOwnership::External, |record| record.ownership),
            reuse_consent,
            current
                .as_ref()
                .map_or(RuntimeConsentState::NotRequested, |record| {
                    record.management_consent
                }),
            unix_timestamp_millis(),
        )
        .map_err(|_| RuntimeError::InvalidInput)?;
        self.save_policy(policy.clone(), &context).await?;
        Ok(compose_report(observation, Some(&policy)))
    }

    /// Returns a bounded model inventory only when current policy permits reuse.
    pub async fn list_models(
        &self,
        provider_id: &RuntimeProviderId,
        limit: u16,
        context: RuntimeOperationContext,
    ) -> Result<RuntimeModelInventory, RuntimeError> {
        self.ensure_provider(provider_id)?;
        if limit == 0 || limit > MAX_MODEL_INVENTORY_ITEMS {
            return Err(RuntimeError::InvalidInput);
        }
        let observation = self.provider.detect(context.clone()).await?;
        let policy = self.load_policy(provider_id, &context).await?;
        ensure_read_access(&observation, policy.as_ref())?;
        let mut inventory = self.provider.list_models(limit, context).await?;
        if inventory.schema_version != RUNTIME_REPORT_SCHEMA_VERSION
            || inventory.provider_id != *provider_id
        {
            return Err(RuntimeError::InvalidResponse);
        }
        let maximum = usize::from(limit.min(MAX_MODEL_INVENTORY_ITEMS));
        if inventory.models.len() > maximum {
            inventory.models.truncate(maximum);
            inventory.truncated = true;
        }
        Ok(inventory)
    }

    /// Executes one bounded lifecycle operation under ownership and consent policy.
    pub async fn execute(
        &self,
        provider_id: &RuntimeProviderId,
        kind: RuntimeOperationKind,
        context: RuntimeOperationContext,
    ) -> Result<RuntimeHealthReport, RuntimeError> {
        self.ensure_provider(provider_id)?;
        if matches!(kind, RuntimeOperationKind::Unknown) {
            return Err(RuntimeError::InvalidInput);
        }
        let observation = self.provider.detect(context.clone()).await?;
        let policy = self.load_policy(provider_id, &context).await?;
        ensure_management_access(&observation, policy.as_ref(), kind)?;
        let _guard = self
            .lifecycle
            .clone()
            .try_lock_owned()
            .map_err(|_| RuntimeError::Busy)?;
        let updated = self.provider.execute(kind, context).await?;
        Ok(compose_report(updated, policy.as_ref()))
    }

    fn ensure_provider(&self, provider_id: &RuntimeProviderId) -> Result<(), RuntimeError> {
        if self.provider.provider_id() == provider_id {
            Ok(())
        } else {
            Err(RuntimeError::InvalidInput)
        }
    }

    async fn load_policy(
        &self,
        provider_id: &RuntimeProviderId,
        context: &RuntimeOperationContext,
    ) -> Result<Option<RuntimePolicyRecord>, RuntimeError> {
        context.check()?;
        let policy = self.policy.clone();
        let provider_id = provider_id.clone();
        let result = tokio::task::spawn_blocking(move || policy.get(&provider_id))
            .await
            .map_err(|_| RuntimeError::PolicyUnavailable)?;
        context.check()?;
        result
    }

    async fn save_policy(
        &self,
        record: RuntimePolicyRecord,
        context: &RuntimeOperationContext,
    ) -> Result<(), RuntimeError> {
        context.check()?;
        let policy = self.policy.clone();
        // The upsert is one short atomic commit point. Once dispatched, await it so a
        // committed consent decision is never reported to the caller as cancelled.
        tokio::task::spawn_blocking(move || policy.upsert(&record))
            .await
            .map_err(|_| RuntimeError::PolicyUnavailable)?
    }
}

fn ensure_reuse_decision(
    observation: &RuntimeObservation,
    policy: Option<&RuntimePolicyRecord>,
    decision: RuntimeConsentDecision,
) -> Result<(), RuntimeError> {
    let ownership = policy.map_or(RuntimeOwnership::External, |record| record.ownership);
    if ownership != RuntimeOwnership::External {
        return Err(RuntimeError::OwnershipConflict);
    }
    if decision == RuntimeConsentDecision::DenyReuse {
        return Ok(());
    }
    match observation.state {
        RuntimeState::Incompatible => return Err(RuntimeError::IncompatibleVersion),
        RuntimeState::Failed => return Err(RuntimeError::ProviderUnavailable),
        RuntimeState::NotInstalled => return Err(RuntimeError::NotInstalled),
        _ => {}
    }
    if observation.endpoint_safety != RuntimeEndpointSafety::LoopbackVerified {
        return Err(RuntimeError::EndpointUnsafe);
    }
    if observation.capabilities.iter().any(|descriptor| {
        descriptor.kind == RuntimeCapabilityKind::ModelInventory
            && descriptor.availability == RuntimeCapabilityAvailability::Available
    }) {
        Ok(())
    } else {
        Err(RuntimeError::Unsupported)
    }
}

fn compose_report(
    observation: RuntimeObservation,
    policy: Option<&RuntimePolicyRecord>,
) -> RuntimeHealthReport {
    let present = observation.state != RuntimeState::NotInstalled;
    let ownership = policy.map_or(
        if present {
            RuntimeOwnership::External
        } else {
            RuntimeOwnership::Unknown
        },
        |record| record.ownership,
    );
    let reuse_consent = policy.map_or(RuntimeConsentState::NotRequested, |record| {
        record.reuse_consent
    });
    let management_consent = policy.map_or(RuntimeConsentState::NotRequested, |record| {
        record.management_consent
    });
    let capabilities = observation
        .capabilities
        .into_iter()
        .map(|descriptor| {
            apply_policy_to_capability(
                descriptor,
                observation.state,
                ownership,
                reuse_consent,
                management_consent,
            )
        })
        .collect();
    let mut warnings = observation.warnings;
    if present
        && ownership == RuntimeOwnership::External
        && !warnings
            .iter()
            .any(|warning| warning.code == RuntimeWarningCode::ExternalInstallation)
    {
        warnings.push(RuntimeWarning {
            code: RuntimeWarningCode::ExternalInstallation,
            message: "The detected runtime remains externally managed.".to_owned(),
        });
    }

    RuntimeHealthReport {
        schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
        provider_id: observation.provider_id,
        display_name: observation.display_name,
        state: observation.state,
        ownership,
        reuse_consent,
        management_consent,
        endpoint_safety: observation.endpoint_safety,
        version: observation.version,
        capabilities,
        reasons: observation.reasons,
        warnings,
    }
}

fn apply_policy_to_capability(
    mut descriptor: RuntimeCapabilityDescriptor,
    state: RuntimeState,
    ownership: RuntimeOwnership,
    reuse_consent: RuntimeConsentState,
    management_consent: RuntimeConsentState,
) -> RuntimeCapabilityDescriptor {
    if descriptor.availability != RuntimeCapabilityAvailability::Available {
        return descriptor;
    }
    descriptor.availability = match descriptor.kind {
        RuntimeCapabilityKind::ModelInventory
            if state == RuntimeState::NotInstalled || state == RuntimeState::Incompatible =>
        {
            RuntimeCapabilityAvailability::Unsupported
        }
        RuntimeCapabilityKind::ModelInventory
            if ownership == RuntimeOwnership::External
                && reuse_consent != RuntimeConsentState::ReuseApproved =>
        {
            RuntimeCapabilityAvailability::RequiresReuseConsent
        }
        RuntimeCapabilityKind::Start
        | RuntimeCapabilityKind::Stop
        | RuntimeCapabilityKind::Restart
            if !matches!(
                ownership,
                RuntimeOwnership::GixGizManaged | RuntimeOwnership::Bundled
            ) || management_consent != RuntimeConsentState::ManagementApproved =>
        {
            RuntimeCapabilityAvailability::RequiresManagementConsent
        }
        _ => RuntimeCapabilityAvailability::Available,
    };
    descriptor
}

fn ensure_read_access(
    observation: &RuntimeObservation,
    policy: Option<&RuntimePolicyRecord>,
) -> Result<(), RuntimeError> {
    match observation.state {
        RuntimeState::NotInstalled => return Err(RuntimeError::NotInstalled),
        RuntimeState::Incompatible => return Err(RuntimeError::IncompatibleVersion),
        RuntimeState::Failed => return Err(RuntimeError::ProviderUnavailable),
        _ => {}
    }
    if observation.endpoint_safety != RuntimeEndpointSafety::LoopbackVerified {
        return Err(RuntimeError::EndpointUnsafe);
    }
    let ownership = policy.map_or(RuntimeOwnership::External, |record| record.ownership);
    let reuse = policy.map_or(RuntimeConsentState::NotRequested, |record| {
        record.reuse_consent
    });
    if ownership == RuntimeOwnership::External && reuse != RuntimeConsentState::ReuseApproved {
        return Err(RuntimeError::ConsentRequired);
    }
    Ok(())
}

fn ensure_management_access(
    observation: &RuntimeObservation,
    policy: Option<&RuntimePolicyRecord>,
    kind: RuntimeOperationKind,
) -> Result<(), RuntimeError> {
    if observation.endpoint_safety != RuntimeEndpointSafety::LoopbackVerified {
        return Err(RuntimeError::EndpointUnsafe);
    }
    let Some(policy) = policy else {
        return Err(RuntimeError::OwnershipConflict);
    };
    if !matches!(
        policy.ownership,
        RuntimeOwnership::GixGizManaged | RuntimeOwnership::Bundled
    ) {
        return Err(RuntimeError::OwnershipConflict);
    }
    if policy.management_consent != RuntimeConsentState::ManagementApproved {
        return Err(RuntimeError::ConsentRequired);
    }
    let capability = match kind {
        RuntimeOperationKind::Start => RuntimeCapabilityKind::Start,
        RuntimeOperationKind::Stop => RuntimeCapabilityKind::Stop,
        RuntimeOperationKind::Restart => RuntimeCapabilityKind::Restart,
        RuntimeOperationKind::Unknown => return Err(RuntimeError::InvalidInput),
        _ => return Err(RuntimeError::InvalidInput),
    };
    if observation.capabilities.iter().any(|descriptor| {
        descriptor.kind == capability
            && descriptor.availability == RuntimeCapabilityAvailability::Available
    }) {
        Ok(())
    } else {
        Err(RuntimeError::Unsupported)
    }
}

fn unix_timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use gixgiz_contracts::{
        CorrelationId, RequestId, RuntimeDisplayName, RuntimeModelInventory,
        RuntimeModelMappingStatus, RuntimeModelSummary, RuntimeProviderId, RuntimeProviderModelId,
        RuntimeProviderModelMapping, RuntimeReason, RuntimeReasonCode,
    };
    use gixgiz_persistence::DataRoot;
    use gixgiz_runtime::testing::{FakeRuntimeCall, FakeRuntimeProvider};

    use super::*;

    fn context() -> RuntimeOperationContext {
        RuntimeOperationContext::new(
            CorrelationId::new(),
            RequestId::new(),
            Duration::from_secs(1),
        )
    }

    fn observation(state: RuntimeState) -> RuntimeObservation {
        RuntimeObservation {
            provider_id: RuntimeProviderId::new("test.runtime"),
            display_name: RuntimeDisplayName::new("Test runtime"),
            state,
            endpoint_safety: RuntimeEndpointSafety::LoopbackVerified,
            version: None,
            capabilities: [
                RuntimeCapabilityKind::Detection,
                RuntimeCapabilityKind::Health,
                RuntimeCapabilityKind::Version,
                RuntimeCapabilityKind::Start,
                RuntimeCapabilityKind::Stop,
                RuntimeCapabilityKind::Restart,
                RuntimeCapabilityKind::ModelInventory,
            ]
            .into_iter()
            .map(|kind| RuntimeCapabilityDescriptor {
                kind,
                availability: RuntimeCapabilityAvailability::Available,
                reason: None,
            })
            .collect(),
            reasons: vec![RuntimeReason {
                code: RuntimeReasonCode::EndpointReachable,
                message: "The test endpoint is reachable.".to_owned(),
            }],
            warnings: Vec::new(),
        }
    }

    fn provider(state: RuntimeState) -> Arc<FakeRuntimeProvider> {
        let observation = observation(state);
        Arc::new(FakeRuntimeProvider::new(
            observation.provider_id.clone(),
            Ok(observation.clone()),
            Ok(observation.clone()),
            Ok(RuntimeModelInventory {
                schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
                provider_id: observation.provider_id,
                models: Vec::new(),
                truncated: false,
                collected_at_unix_ms: 1,
            }),
        ))
    }

    fn model(provider_model_id: &str) -> RuntimeModelSummary {
        RuntimeModelSummary {
            provider_model_id: RuntimeProviderModelId::new(provider_model_id),
            display_name: provider_model_id.to_owned(),
            size_bytes: Some(1),
            mapping: RuntimeProviderModelMapping {
                status: RuntimeModelMappingStatus::External,
                catalogue_id: None,
            },
        }
    }

    #[tokio::test]
    async fn detection_defaults_to_external_without_writing_policy() {
        let provider = provider(RuntimeState::Ready);
        let service = RuntimeService::in_memory(provider.clone());
        let report = service
            .status(provider.provider_id(), context())
            .await
            .expect("status succeeds");

        assert_eq!(report.ownership, RuntimeOwnership::External);
        assert_eq!(report.reuse_consent, RuntimeConsentState::NotRequested);
        assert_eq!(provider.calls(), vec![FakeRuntimeCall::Detect]);
    }

    #[tokio::test]
    async fn reuse_approval_does_not_transfer_ownership_or_management() {
        let provider = provider(RuntimeState::Ready);
        let service = RuntimeService::in_memory(provider.clone());
        let report = service
            .set_reuse_consent(
                provider.provider_id(),
                RuntimeConsentDecision::ApproveReuse,
                context(),
            )
            .await
            .expect("consent records");

        assert_eq!(report.ownership, RuntimeOwnership::External);
        assert_eq!(report.reuse_consent, RuntimeConsentState::ReuseApproved);
        assert_eq!(report.management_consent, RuntimeConsentState::NotRequested);
        assert_eq!(
            service
                .execute(
                    provider.provider_id(),
                    RuntimeOperationKind::Start,
                    context(),
                )
                .await,
            Err(RuntimeError::OwnershipConflict)
        );
    }

    #[tokio::test]
    async fn model_inventory_requires_external_reuse_consent() {
        let provider = provider(RuntimeState::Ready);
        let service = RuntimeService::in_memory(provider.clone());

        assert_eq!(
            service
                .list_models(provider.provider_id(), 10, context())
                .await,
            Err(RuntimeError::ConsentRequired)
        );
        service
            .set_reuse_consent(
                provider.provider_id(),
                RuntimeConsentDecision::ApproveReuse,
                context(),
            )
            .await
            .expect("reuse approved");
        assert!(
            service
                .list_models(provider.provider_id(), 10, context())
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn every_normalized_state_is_preserved_by_fake_detection() {
        for state in [
            RuntimeState::NotInstalled,
            RuntimeState::InstalledStopped,
            RuntimeState::Starting,
            RuntimeState::Ready,
            RuntimeState::Degraded,
            RuntimeState::Incompatible,
            RuntimeState::Updating,
            RuntimeState::Failed,
        ] {
            let provider = provider(state);
            let report = RuntimeService::in_memory(provider.clone())
                .status(provider.provider_id(), context())
                .await
                .expect("fake status succeeds");
            assert_eq!(report.state, state);
        }
    }

    #[tokio::test]
    async fn managed_and_approved_policy_allows_fake_lifecycle() {
        let provider = provider(RuntimeState::Ready);
        let policy = Arc::new(MemoryRuntimePolicyStore::default());
        policy
            .upsert(
                &RuntimePolicyRecord::new(
                    provider.provider_id().clone(),
                    RuntimeOwnership::GixGizManaged,
                    RuntimeConsentState::NotRequested,
                    RuntimeConsentState::ManagementApproved,
                    1,
                )
                .expect("managed policy is valid"),
            )
            .expect("managed policy stores");
        let service = RuntimeService::new(provider.clone(), policy);

        let report = service
            .execute(
                provider.provider_id(),
                RuntimeOperationKind::Start,
                context(),
            )
            .await
            .expect("managed lifecycle succeeds");

        assert_eq!(report.ownership, RuntimeOwnership::GixGizManaged);
        assert_eq!(
            provider.calls(),
            vec![
                FakeRuntimeCall::Detect,
                FakeRuntimeCall::Lifecycle(RuntimeOperationKind::Start),
            ]
        );
    }

    #[tokio::test]
    async fn reuse_approval_fails_closed_for_unsafe_or_incompatible_evidence() {
        let mut unsafe_observation = observation(RuntimeState::Ready);
        unsafe_observation.endpoint_safety = RuntimeEndpointSafety::Unsafe;
        let unsafe_provider = Arc::new(FakeRuntimeProvider::new(
            unsafe_observation.provider_id.clone(),
            Ok(unsafe_observation.clone()),
            Ok(unsafe_observation.clone()),
            Ok(RuntimeModelInventory {
                schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
                provider_id: unsafe_observation.provider_id.clone(),
                models: Vec::new(),
                truncated: false,
                collected_at_unix_ms: 1,
            }),
        ));
        assert_eq!(
            RuntimeService::in_memory(unsafe_provider.clone())
                .set_reuse_consent(
                    unsafe_provider.provider_id(),
                    RuntimeConsentDecision::ApproveReuse,
                    context(),
                )
                .await,
            Err(RuntimeError::EndpointUnsafe)
        );

        let incompatible_provider = provider(RuntimeState::Incompatible);
        assert_eq!(
            RuntimeService::in_memory(incompatible_provider.clone())
                .set_reuse_consent(
                    incompatible_provider.provider_id(),
                    RuntimeConsentDecision::ApproveReuse,
                    context(),
                )
                .await,
            Err(RuntimeError::IncompatibleVersion)
        );
    }

    #[tokio::test]
    async fn core_validates_and_enforces_provider_inventory_bounds() {
        let observation = observation(RuntimeState::Ready);
        let provider = Arc::new(FakeRuntimeProvider::new(
            observation.provider_id.clone(),
            Ok(observation.clone()),
            Ok(observation.clone()),
            Ok(RuntimeModelInventory {
                schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
                provider_id: observation.provider_id.clone(),
                models: vec![model("one"), model("two")],
                truncated: false,
                collected_at_unix_ms: 1,
            }),
        ));
        let service = RuntimeService::in_memory(provider.clone());
        service
            .set_reuse_consent(
                provider.provider_id(),
                RuntimeConsentDecision::ApproveReuse,
                context(),
            )
            .await
            .expect("reuse is approved");

        let inventory = service
            .list_models(provider.provider_id(), 1, context())
            .await
            .expect("bounded inventory succeeds");
        assert_eq!(inventory.models.len(), 1);
        assert!(inventory.truncated);

        let mismatched = Arc::new(FakeRuntimeProvider::new(
            observation.provider_id.clone(),
            Ok(observation.clone()),
            Ok(observation.clone()),
            Ok(RuntimeModelInventory {
                schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
                provider_id: RuntimeProviderId::new("other.runtime"),
                models: Vec::new(),
                truncated: false,
                collected_at_unix_ms: 1,
            }),
        ));
        let mismatched_service = RuntimeService::in_memory(mismatched.clone());
        mismatched_service
            .set_reuse_consent(
                mismatched.provider_id(),
                RuntimeConsentDecision::ApproveReuse,
                context(),
            )
            .await
            .expect("reuse is approved");
        assert_eq!(
            mismatched_service
                .list_models(mismatched.provider_id(), 1, context())
                .await,
            Err(RuntimeError::InvalidResponse)
        );
    }

    #[tokio::test]
    async fn durable_runtime_policy_survives_core_service_reopen() {
        let temporary = tempfile::tempdir().expect("temporary directory is available");
        let root =
            DataRoot::from_override(temporary.path().join("root")).expect("test root initializes");
        let provider = provider(RuntimeState::Ready);
        let persistence = Persistence::open(root.clone()).expect("persistence opens");
        let service = RuntimeService::with_persistence(provider.clone(), &persistence);
        service
            .set_reuse_consent(
                provider.provider_id(),
                RuntimeConsentDecision::ApproveReuse,
                context(),
            )
            .await
            .expect("reuse policy stores");
        drop(service);
        drop(persistence);

        let reopened = Persistence::open(root).expect("persistence reopens");
        let report = RuntimeService::with_persistence(provider.clone(), &reopened)
            .status(provider.provider_id(), context())
            .await
            .expect("durable policy reloads");
        assert_eq!(report.ownership, RuntimeOwnership::External);
        assert_eq!(report.reuse_consent, RuntimeConsentState::ReuseApproved);
        assert_eq!(report.management_consent, RuntimeConsentState::NotRequested);
    }
}
