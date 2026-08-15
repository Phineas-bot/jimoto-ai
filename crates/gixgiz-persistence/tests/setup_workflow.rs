use gixgiz_contracts::{
    CandidateModelId, CatalogueVersion, CorrelationId, ModelAcquisitionPhase, ModelSizeClass,
    RequestId, RuleSetVersion, RuntimeProviderId, RuntimeProviderModelId,
};
use gixgiz_persistence::{
    CURRENT_SCHEMA_VERSION, DataRoot, JobId, PersistedArtifactUpdate, PersistedDestinationCategory,
    PersistedEffectDisposition, PersistedModelIntegrity, PersistedModelLifecycle,
    PersistedModelVerification, PersistedSetupApprovalDecision, PersistedSetupApprovalInput,
    PersistedSetupEffectInput, PersistedSetupEventKind, PersistedSetupNotice, PersistedSetupPlan,
    PersistedSetupProgress, PersistedSetupStage, PersistedSetupState, PersistedSetupTransition,
    Persistence, PersistenceError,
};
use rusqlite::{Connection, params};

fn test_root(temporary: &tempfile::TempDir) -> DataRoot {
    DataRoot::from_override(temporary.path().join("gixgiz-test-root"))
        .expect("isolated test root initializes")
}

fn setup_plan(job_id: &str) -> PersistedSetupPlan {
    PersistedSetupPlan {
        job_id: JobId::new(job_id).expect("job ID is valid"),
        plan_schema_version: 1,
        plan_revision: 1,
        canonical_model_id: CandidateModelId::new("model.general.compact"),
        model_display_name: "Compact General Model".to_owned(),
        model_family: "general".to_owned(),
        model_size_class: ModelSizeClass::Compact,
        licence_spdx: "Apache-2.0".to_owned(),
        provenance_url: "https://example.invalid/models/compact".to_owned(),
        provider_id: RuntimeProviderId::new("gixgiz.runtime.test.v1"),
        provider_model_id: RuntimeProviderModelId::new("compact:latest"),
        artifact_source_summary: "Validated provider mapping".to_owned(),
        destination_display: "Provider-managed model storage".to_owned(),
        runtime_display_name: "Local runtime".to_owned(),
        runtime_version: Some("1.2.3".to_owned()),
        catalogue_version: CatalogueVersion::new("catalogue-1"),
        rule_set_version: RuleSetVersion::new("rules-1"),
        destination_category: PersistedDestinationCategory::ProviderManaged,
        expected_size_bytes: 4_000,
        expected_memory_bytes: 8_000,
        memory_safety_margin_bytes: 2_000,
        storage_safety_margin_bytes: 1_000,
        planned_context_tokens: 2_048,
        cpu_only: true,
        gpu_memory_bytes: None,
        observed_total_memory_bytes: Some(16_000),
        observed_available_memory_bytes: Some(12_000),
        observed_free_storage_bytes: Some(100_000),
        acceleration: None,
        external_runtime_effect: true,
        notices: vec![PersistedSetupNotice {
            warning: false,
            code: "capability.safe_fit".to_owned(),
            message: "This model preserves the required memory headroom.".to_owned(),
        }],
        effect_codes: vec!["model.acquire_provider_artifact".to_owned()],
        created_at_unix_ms: 10,
    }
}

fn approval(
    plan: &PersistedSetupPlan,
    decision: PersistedSetupApprovalDecision,
    at: i64,
) -> PersistedSetupApprovalInput {
    PersistedSetupApprovalInput {
        job_id: plan.job_id.clone(),
        plan_revision: plan.plan_revision,
        decision,
        correlation_id: CorrelationId::new(),
        request_id: RequestId::new(),
        decided_at_unix_ms: at,
    }
}

fn transition(
    plan: &PersistedSetupPlan,
    expected_state: PersistedSetupState,
    expected_revision: u64,
    next_state: PersistedSetupState,
    next_stage: PersistedSetupStage,
    occurred_at_unix_ms: i64,
) -> PersistedSetupTransition {
    PersistedSetupTransition {
        job_id: plan.job_id.clone(),
        expected_state,
        expected_revision,
        next_state,
        next_stage,
        progress: None,
        clear_progress: false,
        safe_error_code: None,
        attention_reason: None,
        runtime_verified_at_unix_ms: None,
        model_available_at_unix_ms: None,
        registration_verified_at_unix_ms: None,
        inference_verified_at_unix_ms: None,
        clear_verification: false,
        clear_cancellation: false,
        increment_retry: false,
        artifact_update: None,
        effects: Vec::new(),
        event_kind: PersistedSetupEventKind::StageChanged,
        correlation_id: CorrelationId::new(),
        occurred_at_unix_ms,
    }
}

#[test]
fn schema_two_database_upgrades_without_losing_durable_jobs() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    let connection = Connection::open(root.database_path()).expect("database opens");
    connection
        .execute_batch(include_str!("../migrations/0001_initial.sql"))
        .expect("version-one schema initializes");
    connection
        .execute_batch(include_str!("../migrations/0002_runtime_policy.sql"))
        .expect("version-two schema initializes");
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name, applied_at_unix_ms)
             VALUES (1, 'initial', 1), (2, 'runtime_policy', 2)",
            [],
        )
        .expect("prior migration ledger records");
    connection
        .execute(
            "INSERT INTO durable_jobs (id, kind, state, updated_at_unix_ms)
             VALUES ('prior-job', 'foundation', 'queued', 2)",
            [],
        )
        .expect("prior job records");
    connection
        .pragma_update(None, "user_version", 2)
        .expect("version-two marker records");
    drop(connection);

    let persistence = Persistence::open(root).expect("version-two database upgrades");

    assert_eq!(
        persistence
            .health_check()
            .expect("upgraded database is healthy")
            .schema_version,
        CURRENT_SCHEMA_VERSION
    );
    assert!(
        persistence
            .jobs()
            .get(&JobId::new("prior-job").expect("job ID is valid"))
            .expect("prior job reads")
            .is_some()
    );
}

#[test]
fn approval_copies_the_immutable_plan_snapshot() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let plan = setup_plan("setup-approval");
    let repository = persistence.setup_jobs();
    let created = repository
        .create(&plan, CorrelationId::new())
        .expect("setup plan persists");

    assert_eq!(created.state, PersistedSetupState::AwaitingApproval);
    assert_eq!(created.revision, 0);
    assert_eq!(created.last_event_sequence, 1);

    let inspection = Connection::open(persistence.data_root().database_path())
        .expect("inspection connection opens");
    inspection
        .execute(
            "UPDATE models SET licence_spdx = 'MIT', provenance_url = 'https://example.invalid/new'\
             WHERE canonical_model_id = ?1",
            params![plan.canonical_model_id.as_str()],
        )
        .expect("catalogue metadata changes independently");

    repository
        .decide_approval(&approval(
            &plan,
            PersistedSetupApprovalDecision::Approve,
            20,
        ))
        .expect("exact plan revision is approved");

    let approved_scope: (String, String, String) = inspection
        .query_row(
            "SELECT licence_spdx, provenance_url, provider_model_id
             FROM setup_approvals WHERE job_id = ?1",
            params![plan.job_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("approval scope reads");
    assert_eq!(approved_scope.0, plan.licence_spdx);
    assert_eq!(approved_scope.1, plan.provenance_url);
    assert_eq!(approved_scope.2, plan.provider_model_id.as_str());
    let approved_effect: String = inspection
        .query_row(
            "SELECT effect_code FROM setup_approval_effects WHERE job_id = ?1",
            params![plan.job_id.as_str()],
            |row| row.get(0),
        )
        .expect("approved effect reads");
    assert_eq!(approved_effect, plan.effect_codes[0]);

    let reloaded = repository
        .get(&plan.job_id)
        .expect("setup job reads")
        .expect("setup job exists");
    assert_eq!(reloaded.plan.licence_spdx, "Apache-2.0");
    assert_eq!(reloaded.state, PersistedSetupState::Approved);
}

#[test]
fn approval_rejects_a_stale_plan_revision() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let plan = setup_plan("setup-stale-approval");
    let repository = persistence.setup_jobs();
    repository
        .create(&plan, CorrelationId::new())
        .expect("setup plan persists");

    let error = repository
        .decide_approval(&PersistedSetupApprovalInput {
            plan_revision: plan.plan_revision + 1,
            ..approval(&plan, PersistedSetupApprovalDecision::Approve, 20)
        })
        .expect_err("a stale revision must not authorize effects");

    assert!(matches!(
        error,
        PersistenceError::RecordConflict {
            entity: "setup_job"
        }
    ));
    assert_eq!(
        repository
            .get(&plan.job_id)
            .expect("setup job reads")
            .expect("setup job exists")
            .state,
        PersistedSetupState::AwaitingApproval
    );
}

#[test]
fn approval_rejects_a_job_with_durable_cancellation_intent() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let plan = setup_plan("setup-cancel-before-approval");
    let repository = persistence.setup_jobs();
    repository
        .create(&plan, CorrelationId::new())
        .expect("setup plan persists");
    repository
        .request_cancellation(&plan.job_id, CorrelationId::new(), 15)
        .expect("cancellation intent persists");

    assert!(matches!(
        repository.decide_approval(&approval(
            &plan,
            PersistedSetupApprovalDecision::Approve,
            20,
        )),
        Err(PersistenceError::RecordConflict {
            entity: "setup_job"
        })
    ));
    assert!(
        repository
            .approval(&plan.job_id)
            .expect("approval lookup succeeds")
            .is_none()
    );
    let job = repository
        .get(&plan.job_id)
        .expect("setup job reads")
        .expect("setup job exists");
    assert_eq!(job.state, PersistedSetupState::AwaitingApproval);
    assert!(job.cancellation_requested);
}

#[test]
fn denial_is_durable_and_authorizes_no_effects() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let plan = setup_plan("setup-denied");
    let repository = persistence.setup_jobs();
    repository
        .create(&plan, CorrelationId::new())
        .expect("setup plan persists");

    repository
        .decide_approval(&approval(&plan, PersistedSetupApprovalDecision::Deny, 20))
        .expect("denial persists");

    let decision = repository
        .approval(&plan.job_id)
        .expect("decision reads")
        .expect("decision exists");
    let job = repository
        .get(&plan.job_id)
        .expect("setup job reads")
        .expect("setup job exists");
    assert_eq!(decision.decision, PersistedSetupApprovalDecision::Deny);
    assert!(decision.approved_effect_codes.is_empty());
    assert_eq!(job.state, PersistedSetupState::Cancelled);
    assert_eq!(
        repository
            .latest_recoverable()
            .expect("latest durable job reads")
            .expect("denied job remains recoverable")
            .plan
            .job_id,
        plan.job_id
    );
}

#[test]
fn only_one_nonterminal_setup_job_can_exist() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let repository = persistence.setup_jobs();
    let first = setup_plan("setup-first-active");
    let second = setup_plan("setup-second-active");

    repository
        .create(&first, CorrelationId::new())
        .expect("first setup plan persists");

    assert!(matches!(
        repository.create(&second, CorrelationId::new()),
        Err(PersistenceError::RecordConflict {
            entity: "setup_job"
        })
    ));
    assert!(
        repository
            .get(&second.job_id)
            .expect("second setup job lookup succeeds")
            .is_none()
    );
}

#[test]
fn ready_requires_all_evidence_and_marks_the_artifact_atomically() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let plan = setup_plan("setup-ready");
    let repository = persistence.setup_jobs();
    repository
        .create(&plan, CorrelationId::new())
        .expect("setup plan persists");
    let approved = repository
        .decide_approval(&approval(
            &plan,
            PersistedSetupApprovalDecision::Approve,
            20,
        ))
        .expect("setup plan is approved");
    let incomplete = transition(
        &plan,
        PersistedSetupState::Approved,
        approved.revision,
        PersistedSetupState::Ready,
        PersistedSetupStage::Complete,
        30,
    );

    assert!(matches!(
        repository.transition(&incomplete),
        Err(PersistenceError::InvalidRecord {
            field: "setup_ready_evidence"
        })
    ));

    let mut ready = incomplete;
    ready.runtime_verified_at_unix_ms = Some(30);
    ready.model_available_at_unix_ms = Some(30);
    ready.registration_verified_at_unix_ms = Some(30);
    ready.inference_verified_at_unix_ms = Some(30);
    ready.event_kind = PersistedSetupEventKind::Terminal;
    ready.artifact_update = Some(PersistedArtifactUpdate {
        lifecycle: PersistedModelLifecycle::Available,
        verification: PersistedModelVerification::Verified,
        integrity: PersistedModelIntegrity::ProviderReported,
        measured_size_bytes: Some(3_900),
        registered_at_unix_ms: Some(30),
        verified_at_unix_ms: Some(30),
    });
    repository
        .transition(&ready)
        .expect("all readiness evidence commits atomically");

    let job = repository
        .get(&plan.job_id)
        .expect("setup job reads")
        .expect("setup job exists");
    let artifact = repository
        .artifact(&plan.canonical_model_id, &plan.provider_id)
        .expect("model artifact reads")
        .expect("model artifact exists");
    assert_eq!(job.state, PersistedSetupState::Ready);
    assert_eq!(artifact.lifecycle, PersistedModelLifecycle::Available);
    assert_eq!(artifact.verification, PersistedModelVerification::Verified);
}

#[test]
fn ready_rejects_mismatched_artifact_integrity_atomically() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let plan = setup_plan("setup-integrity-mismatch");
    let repository = persistence.setup_jobs();
    repository
        .create(&plan, CorrelationId::new())
        .expect("setup plan persists");
    let approved = repository
        .decide_approval(&approval(
            &plan,
            PersistedSetupApprovalDecision::Approve,
            20,
        ))
        .expect("setup plan is approved");
    let mut ready = transition(
        &plan,
        PersistedSetupState::Approved,
        approved.revision,
        PersistedSetupState::Ready,
        PersistedSetupStage::Complete,
        30,
    );
    ready.runtime_verified_at_unix_ms = Some(30);
    ready.model_available_at_unix_ms = Some(30);
    ready.registration_verified_at_unix_ms = Some(30);
    ready.inference_verified_at_unix_ms = Some(30);
    ready.event_kind = PersistedSetupEventKind::Terminal;
    ready.artifact_update = Some(PersistedArtifactUpdate {
        lifecycle: PersistedModelLifecycle::Available,
        verification: PersistedModelVerification::Verified,
        integrity: PersistedModelIntegrity::Mismatch,
        measured_size_bytes: Some(3_900),
        registered_at_unix_ms: Some(30),
        verified_at_unix_ms: Some(30),
    });

    assert!(matches!(
        repository.transition(&ready),
        Err(PersistenceError::InvalidRecord {
            field: "model_artifact_update" | "setup_ready_evidence"
        })
    ));
    let job = repository
        .get(&plan.job_id)
        .expect("setup job reads")
        .expect("setup job exists");
    let artifact = repository
        .artifact(&plan.canonical_model_id, &plan.provider_id)
        .expect("model artifact reads")
        .expect("model artifact exists");
    assert_eq!(job.state, PersistedSetupState::Approved);
    assert_eq!(artifact.lifecycle, PersistedModelLifecycle::Planned);
    assert_eq!(
        artifact.verification,
        PersistedModelVerification::NotStarted
    );
}

#[test]
fn progress_is_monotonic_and_failed_writes_are_atomic() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let plan = setup_plan("setup-progress");
    let repository = persistence.setup_jobs();
    repository
        .create(&plan, CorrelationId::new())
        .expect("setup plan persists");
    let approved = repository
        .decide_approval(&approval(
            &plan,
            PersistedSetupApprovalDecision::Approve,
            20,
        ))
        .expect("setup plan is approved");
    let mut acquiring = transition(
        &plan,
        PersistedSetupState::Approved,
        approved.revision,
        PersistedSetupState::Acquiring,
        PersistedSetupStage::Acquisition,
        30,
    );
    acquiring.progress = Some(PersistedSetupProgress {
        phase: Some(ModelAcquisitionPhase::Transferring),
        completed_bytes: Some(50),
        total_bytes: Some(100),
        basis_points: Some(5_000),
    });
    acquiring.event_kind = PersistedSetupEventKind::Progress;
    let acquired = repository
        .transition(&acquiring)
        .expect("progress persists");
    let mut regressed = transition(
        &plan,
        PersistedSetupState::Acquiring,
        acquired.revision,
        PersistedSetupState::Acquiring,
        PersistedSetupStage::Acquisition,
        31,
    );
    regressed.progress = Some(PersistedSetupProgress {
        phase: Some(ModelAcquisitionPhase::Transferring),
        completed_bytes: Some(40),
        total_bytes: Some(100),
        basis_points: Some(4_000),
    });

    assert!(matches!(
        repository.transition(&regressed),
        Err(PersistenceError::InvalidRecord {
            field: "setup_progress_regression"
        })
    ));
    let observed = repository
        .get(&plan.job_id)
        .expect("setup job reads")
        .expect("setup job exists");
    assert_eq!(observed.revision, acquired.revision);
    assert_eq!(observed.progress.completed_bytes, Some(50));
}

#[test]
fn progress_events_advance_the_cursor_and_event_retention_is_bounded() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let plan = setup_plan("setup-event-retention");
    let repository = persistence.setup_jobs();
    repository
        .create(&plan, CorrelationId::new())
        .expect("setup plan persists");
    let approved = repository
        .decide_approval(&approval(
            &plan,
            PersistedSetupApprovalDecision::Approve,
            20,
        ))
        .expect("setup plan is approved");
    let mut revision = approved.revision;
    let mut expected_state = PersistedSetupState::Approved;
    let mut latest_progress_sequence = approved.event_sequence;
    for completed in 0_u64..=100 {
        let mut progress = transition(
            &plan,
            expected_state,
            revision,
            PersistedSetupState::Acquiring,
            PersistedSetupStage::Acquisition,
            30 + i64::try_from(completed).expect("test counter fits"),
        );
        progress.progress = Some(PersistedSetupProgress {
            phase: Some(ModelAcquisitionPhase::Transferring),
            completed_bytes: Some(completed),
            total_bytes: Some(100),
            basis_points: Some(u16::try_from(completed * 100).expect("basis points fit")),
        });
        progress.event_kind = PersistedSetupEventKind::Progress;
        let result = repository
            .transition(&progress)
            .expect("monotonic progress persists");
        revision = result.revision;
        expected_state = PersistedSetupState::Acquiring;
        assert!(result.event_sequence > latest_progress_sequence);
        latest_progress_sequence = result.event_sequence;
    }

    assert_eq!(
        repository
            .events_after(&plan.job_id, 0, 256)
            .expect("progress events read")
            .len(),
        103
    );

    for offset in 0_u64..300 {
        let event = transition(
            &plan,
            PersistedSetupState::Acquiring,
            revision,
            PersistedSetupState::Acquiring,
            PersistedSetupStage::Acquisition,
            200 + i64::try_from(offset).expect("test counter fits"),
        );
        revision = repository
            .transition(&event)
            .expect("bounded stage event persists")
            .revision;
    }
    let retained = repository
        .events_after(&plan.job_id, 0, 256)
        .expect("retained events read");
    assert_eq!(retained.len(), 256);
    assert!(retained[0].sequence > 1);
}

#[test]
fn cancellation_effects_and_recovery_state_survive_restart() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    let plan = setup_plan("setup-cancelled");
    {
        let persistence = Persistence::open(root.clone()).expect("database opens");
        let repository = persistence.setup_jobs();
        repository
            .create(&plan, CorrelationId::new())
            .expect("setup plan persists");
        let approved = repository
            .decide_approval(&approval(
                &plan,
                PersistedSetupApprovalDecision::Approve,
                20,
            ))
            .expect("setup plan is approved");
        let acquiring = transition(
            &plan,
            PersistedSetupState::Approved,
            approved.revision,
            PersistedSetupState::Acquiring,
            PersistedSetupStage::Acquisition,
            30,
        );
        let acquiring = repository
            .transition(&acquiring)
            .expect("acquisition state persists");
        assert_eq!(
            repository
                .interrupted_jobs()
                .expect("interrupted jobs read")
                .len(),
            1
        );
        let cancellation = repository
            .request_cancellation(&plan.job_id, CorrelationId::new(), 31)
            .expect("cancellation intent persists first");
        assert_eq!(cancellation.revision, acquiring.revision + 1);
        let mut cancelled = transition(
            &plan,
            PersistedSetupState::Acquiring,
            cancellation.revision,
            PersistedSetupState::Cancelled,
            PersistedSetupStage::Cancelled,
            32,
        );
        cancelled.effects.push(PersistedSetupEffectInput {
            disposition: PersistedEffectDisposition::Retained,
            effect_code: "model.provider_partial_retained".to_owned(),
            size_bytes: Some(50),
        });
        cancelled.event_kind = PersistedSetupEventKind::Terminal;
        repository
            .transition(&cancelled)
            .expect("cancelled outcome persists");
    }

    let reopened = Persistence::open(root).expect("database reopens");
    let repository = reopened.setup_jobs();
    let job = repository
        .get(&plan.job_id)
        .expect("setup job reads")
        .expect("setup job exists");
    let effects = repository
        .effects(&plan.job_id)
        .expect("effect report reads");
    let events = repository
        .events_after(&plan.job_id, 0, 32)
        .expect("event replay reads");
    assert_eq!(job.state, PersistedSetupState::Cancelled);
    assert!(job.cancellation_requested);
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].disposition, PersistedEffectDisposition::Retained);
    assert!(events.iter().any(|event| {
        event.kind == PersistedSetupEventKind::CancellationRequested
            && event.safe_code.as_deref() == Some("setup.cancellation_requested")
    }));
}

#[test]
fn setup_schema_and_validation_exclude_private_payloads_and_raw_paths() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let mut invalid_plan = setup_plan("setup-private-boundary");
    invalid_plan.provenance_url = "file:///private/model-card".to_owned();
    assert!(matches!(
        persistence
            .setup_jobs()
            .create(&invalid_plan, CorrelationId::new()),
        Err(PersistenceError::InvalidRecord {
            field: "provenance_url"
        })
    ));

    let inspection = Connection::open(persistence.data_root().database_path())
        .expect("inspection connection opens");
    for table in [
        "setup_jobs",
        "setup_approvals",
        "setup_job_events",
        "setup_effects",
        "model_provider_artifacts",
    ] {
        let columns: Vec<String> = inspection
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("schema statement prepares")
            .query_map([], |row| row.get(1))
            .expect("schema rows query")
            .collect::<Result<_, _>>()
            .expect("schema rows decode");
        assert!(!columns.iter().any(|column| {
            matches!(
                column.as_str(),
                "prompt"
                    | "output"
                    | "content"
                    | "raw_error"
                    | "error_message"
                    | "destination_path"
                    | "staging_path"
                    | "artifact_path"
            )
        }));
    }
}
