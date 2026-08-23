use std::{
    fs, thread,
    time::{Duration, Instant},
};

use gixgiz_contracts::{
    CorrelationId, ErrorCategory, RequestId, RuntimeConsentState, RuntimeOwnership,
    RuntimeProviderId,
};
use gixgiz_persistence::{
    CURRENT_SCHEMA_VERSION, DataRoot, JobId, JobMetadata, JobState, Persistence, PersistenceError,
    PersistenceOptions, RuntimePolicyRecord,
};
use rusqlite::{Connection, TransactionBehavior, params};

fn test_root(temporary: &tempfile::TempDir) -> DataRoot {
    DataRoot::from_override(temporary.path().join("gixgiz-test-root"))
        .expect("isolated test root initializes")
}

#[test]
fn fresh_database_is_configured_and_records_schema_version() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("fresh database opens");

    assert_eq!(
        persistence
            .health_check()
            .expect("health check succeeds")
            .schema_version,
        CURRENT_SCHEMA_VERSION
    );
    let configuration = persistence.configuration().expect("configuration reads");
    assert!(configuration.foreign_keys);
    assert_eq!(configuration.journal_mode, "wal");
    assert_eq!(configuration.busy_timeout_millis, 5_000);
    assert_eq!(
        persistence
            .platform_metadata()
            .get("last_migrated_application_version")
            .expect("migration application version reads")
            .as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn fresh_database_has_bounded_runtime_policy_schema() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("fresh database opens");
    let connection = Connection::open(persistence.data_root().database_path())
        .expect("inspection connection opens");

    let columns: Vec<String> = connection
        .prepare("PRAGMA table_info(runtime_policy)")
        .expect("schema statement prepares")
        .query_map([], |row| row.get(1))
        .expect("schema rows query")
        .collect::<Result<_, _>>()
        .expect("schema rows decode");

    assert_eq!(
        columns,
        vec![
            "provider_id",
            "ownership",
            "reuse_consent",
            "management_consent",
            "updated_at_unix_ms",
            "acknowledged_untested_version",
        ]
    );
}

#[test]
fn schema_one_database_upgrades_without_losing_existing_data() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    let connection = Connection::open(root.database_path()).expect("database opens");
    connection
        .execute_batch(include_str!("../migrations/0001_initial.sql"))
        .expect("version-one schema initializes");
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name, applied_at_unix_ms)
             VALUES (1, 'initial', 1)",
            [],
        )
        .expect("version-one ledger records");
    connection
        .execute(
            "INSERT INTO application_metadata (key, value, updated_at_unix_ms)
             VALUES ('last_migrated_application_version', '0.1.0', 1)",
            [],
        )
        .expect("version-one metadata records");
    connection
        .execute(
            "INSERT INTO settings (key, value, updated_at_unix_ms)
             VALUES ('appearance', 'system', 2)",
            [],
        )
        .expect("version-one setting records");
    connection
        .pragma_update(None, "user_version", 1)
        .expect("version-one marker records");
    drop(connection);

    let persistence = Persistence::open(root).expect("version-one database upgrades");
    assert_eq!(
        persistence
            .health_check()
            .expect("upgraded database is healthy")
            .schema_version,
        CURRENT_SCHEMA_VERSION
    );
    assert_eq!(
        persistence
            .settings()
            .get("appearance")
            .expect("prior setting reads")
            .as_deref(),
        Some("system")
    );
    assert_eq!(
        persistence
            .runtime_policy()
            .get(&RuntimeProviderId::new("local-runtime"))
            .expect("runtime policy repository reads"),
        None
    );
    let inspection = Connection::open(persistence.data_root().database_path())
        .expect("upgraded database inspection opens");
    let versions: Vec<u32> = inspection
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .expect("migration ledger statement prepares")
        .query_map([], |row| row.get(0))
        .expect("migration ledger rows query")
        .collect::<Result<_, _>>()
        .expect("migration ledger rows decode");
    assert_eq!(versions, vec![1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(
        fs::read_dir(persistence.data_root().backups_dir())
            .expect("backup directory reads")
            .count(),
        0,
        "the additive migration must not create an irreversible-migration backup"
    );
}

#[test]
fn runtime_policy_upsert_keeps_ownership_and_consent_separate() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let provider_id = RuntimeProviderId::new("local-runtime");
    let repository = persistence.runtime_policy();
    let initial = RuntimePolicyRecord::new(
        provider_id.clone(),
        RuntimeOwnership::External,
        RuntimeConsentState::ReuseApproved,
        RuntimeConsentState::NotRequested,
        10,
    )
    .expect("initial policy is valid");
    repository.upsert(&initial).expect("initial policy writes");
    assert_eq!(
        repository.get(&provider_id).expect("policy reads"),
        Some(initial)
    );

    let updated = RuntimePolicyRecord::new(
        provider_id.clone(),
        RuntimeOwnership::External,
        RuntimeConsentState::ReuseApproved,
        RuntimeConsentState::ManagementApproved,
        11,
    )
    .expect("updated policy is valid");
    repository.upsert(&updated).expect("policy updates");
    assert_eq!(
        repository.get(&provider_id).expect("updated policy reads"),
        Some(updated)
    );

    assert!(matches!(
        RuntimePolicyRecord::new(
            provider_id.clone(),
            RuntimeOwnership::External,
            RuntimeConsentState::ManagementApproved,
            RuntimeConsentState::NotRequested,
            12,
        ),
        Err(PersistenceError::InvalidRecord {
            field: "runtime_reuse_consent"
        })
    ));
    assert!(matches!(
        RuntimePolicyRecord::new(
            provider_id,
            RuntimeOwnership::External,
            RuntimeConsentState::NotRequested,
            RuntimeConsentState::ReuseApproved,
            12,
        ),
        Err(PersistenceError::InvalidRecord {
            field: "runtime_management_consent"
        })
    ));
}

#[test]
fn reopening_preserves_runtime_policy() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    let provider_id = RuntimeProviderId::new("local-runtime");
    let policy = RuntimePolicyRecord::new(
        provider_id.clone(),
        RuntimeOwnership::GixGizManaged,
        RuntimeConsentState::ReuseApproved,
        RuntimeConsentState::ManagementApproved,
        20,
    )
    .expect("policy is valid");
    {
        let persistence = Persistence::open(root.clone()).expect("database opens");
        persistence
            .runtime_policy()
            .upsert(&policy)
            .expect("policy writes");
    }

    let reopened = Persistence::open(root).expect("database reopens");
    assert_eq!(
        reopened
            .runtime_policy()
            .get(&provider_id)
            .expect("policy reads after reopen"),
        Some(policy)
    );
}

#[test]
fn malformed_runtime_policy_record_fails_closed() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let provider_id = RuntimeProviderId::new("local-runtime");
    persistence
        .runtime_policy()
        .upsert(
            &RuntimePolicyRecord::new(
                provider_id.clone(),
                RuntimeOwnership::External,
                RuntimeConsentState::NotRequested,
                RuntimeConsentState::NotRequested,
                1,
            )
            .expect("policy is valid"),
        )
        .expect("policy writes");

    let connection = Connection::open(persistence.data_root().database_path())
        .expect("inspection connection opens");
    connection
        .pragma_update(None, "ignore_check_constraints", true)
        .expect("test fixture can bypass schema checks");
    connection
        .execute(
            "UPDATE runtime_policy SET ownership = ?1 WHERE provider_id = ?2",
            params!["future_unrecognized_ownership", provider_id.as_str()],
        )
        .expect("malformed fixture writes");
    drop(connection);

    assert!(matches!(
        persistence.runtime_policy().get(&provider_id),
        Err(PersistenceError::InvalidRecord {
            field: "runtime_ownership"
        })
    ));
}

#[test]
fn reopening_preserves_settings_jobs_metadata_and_audit_events() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    let correlation_id = CorrelationId::new();
    let job_id = JobId::new("job-1").expect("job ID is valid");
    {
        let persistence = Persistence::open(root.clone()).expect("database opens");
        persistence
            .platform_metadata()
            .set("installation_state", "initialized", 1)
            .expect("metadata writes");
        persistence
            .settings()
            .set("appearance", "system", 2)
            .expect("setting writes");
        persistence
            .jobs()
            .upsert(
                &JobMetadata::new(job_id.clone(), "foundation", JobState::Queued, 3)
                    .expect("job is valid"),
            )
            .expect("job writes");
        persistence
            .audit_events()
            .append(
                "platform",
                "database_initialized",
                "succeeded",
                correlation_id,
                4,
            )
            .expect("audit event writes");
    }

    let reopened = Persistence::open(root).expect("database reopens");
    assert_eq!(
        reopened
            .platform_metadata()
            .get("installation_state")
            .expect("metadata reads")
            .as_deref(),
        Some("initialized")
    );
    assert_eq!(
        reopened
            .settings()
            .get("appearance")
            .expect("setting reads")
            .as_deref(),
        Some("system")
    );
    assert_eq!(
        reopened.jobs().get(&job_id).expect("job reads"),
        Some(JobMetadata::new(job_id, "foundation", JobState::Queued, 3).expect("job is valid"))
    );
    let events = reopened
        .audit_events()
        .list_recent(100)
        .expect("audit events read");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].correlation_id, correlation_id);
}

#[test]
fn repository_updates_are_transactional_and_prepared() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let id = JobId::generated();
    let jobs = persistence.jobs();
    jobs.upsert(
        &JobMetadata::new(id.clone(), "foundation", JobState::Queued, 1).expect("job is valid"),
    )
    .expect("job creates");
    jobs.upsert(
        &JobMetadata::new(id.clone(), "foundation", JobState::Running, 2).expect("job is valid"),
    )
    .expect("job updates");

    let observed = jobs.get(&id).expect("job reads").expect("job exists");
    assert_eq!(observed.state, JobState::Running);
    assert_eq!(observed.updated_at_unix_ms, 2);
}

#[test]
fn newer_schema_is_refused_without_downgrade() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    let connection = Connection::open(root.database_path()).expect("database opens");
    connection
        .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
        .expect("newer version is written");
    drop(connection);

    let error = Persistence::open(root)
        .err()
        .expect("newer schema must be refused");
    assert!(matches!(
        error,
        PersistenceError::SchemaTooNew {
            found,
            supported
        } if found == CURRENT_SCHEMA_VERSION + 1 && supported == CURRENT_SCHEMA_VERSION
    ));
}

#[test]
fn a_second_platform_owner_is_refused() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    let first = Persistence::open(root.clone()).expect("first owner opens");

    let error = Persistence::open(root)
        .err()
        .expect("second owner must fail");

    assert!(matches!(error, PersistenceError::AlreadyOwned { .. }));
    drop(first);
}

#[test]
fn external_write_contention_honors_the_bounded_busy_timeout() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    let persistence = Persistence::open_with_options(
        root.clone(),
        PersistenceOptions::new(Duration::from_millis(25)),
    )
    .expect("database opens");
    let mut external = Connection::open(root.database_path()).expect("external connection opens");
    let transaction = external
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("external writer acquires lock");

    let error = persistence
        .settings()
        .set("appearance", "system", 1)
        .expect_err("bounded write must time out while locked");

    assert!(matches!(&error, PersistenceError::Locked { .. }));
    let payload = error.to_safe_payload(CorrelationId::new(), RequestId::new());
    assert_eq!(payload.category, ErrorCategory::Unavailable);
    assert_eq!(payload.code, "persistence.locked");
    transaction.rollback().expect("external lock releases");
}

#[test]
fn runtime_policy_write_fails_before_consent_can_outlive_a_request() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    let persistence = Persistence::open(root.clone()).expect("database opens");
    let mut external = Connection::open(root.database_path()).expect("external connection opens");
    let transaction = external
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("external writer acquires lock");
    let provider_id = RuntimeProviderId::new("gixgiz.runtime.test.v1");
    let record = RuntimePolicyRecord::new(
        provider_id.clone(),
        RuntimeOwnership::External,
        RuntimeConsentState::ReuseApproved,
        RuntimeConsentState::NotRequested,
        1,
    )
    .expect("policy record is valid");

    let started = Instant::now();
    let error = persistence
        .runtime_policy()
        .upsert(&record)
        .expect_err("consent write must fail immediately while locked");

    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(matches!(error, PersistenceError::Locked { .. }));
    transaction.rollback().expect("external lock releases");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        persistence
            .runtime_policy()
            .get(&provider_id)
            .expect("policy lookup succeeds"),
        None
    );
    assert_eq!(
        persistence
            .configuration()
            .expect("configuration reads")
            .busy_timeout_millis,
        5_000
    );
}

#[test]
fn corrupt_database_is_detected_and_safely_mapped() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let root = test_root(&temporary);
    fs::write(root.database_path(), b"not a sqlite database").expect("corrupt test fixture writes");

    let error = Persistence::open(root)
        .err()
        .expect("corrupt database must fail");
    let payload = error.to_safe_payload(CorrelationId::new(), RequestId::new());

    assert!(matches!(error, PersistenceError::Corrupt { .. }));
    assert_eq!(payload.category, ErrorCategory::IntegrityFailure);
    assert_eq!(payload.code, "persistence.corrupt");
    assert!(!payload.to_string().contains("sqlite"));
}

#[test]
fn foundation_rejects_secret_keys_large_values_and_private_audit_payloads() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");

    assert!(
        persistence
            .settings()
            .set("api_token", "do-not-store", 1)
            .is_err()
    );
    assert!(
        persistence
            .settings()
            .set("large_value", &"x".repeat(4097), 1)
            .is_err()
    );
    persistence
        .audit_events()
        .append(
            "platform",
            "safe_action",
            "succeeded",
            CorrelationId::new(),
            1,
        )
        .expect("categorical audit event is accepted");

    let connection = Connection::open(persistence.data_root().database_path())
        .expect("inspection connection opens");
    let audit_columns: Vec<String> = connection
        .prepare("PRAGMA table_info(audit_events)")
        .expect("schema statement prepares")
        .query_map([], |row| row.get(1))
        .expect("schema rows query")
        .collect::<Result<_, _>>()
        .expect("schema rows decode");
    assert!(!audit_columns.iter().any(|column| {
        matches!(
            column.as_str(),
            "message" | "payload" | "content" | "secret"
        )
    }));
}

#[test]
fn migration_five_adds_a_nullable_untested_version_acknowledgement() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");

    assert_eq!(
        persistence
            .health_check()
            .expect("health check succeeds")
            .schema_version,
        CURRENT_SCHEMA_VERSION
    );

    let provider_id = RuntimeProviderId::new("test.runtime");
    let policy = RuntimePolicyRecord::new(
        provider_id.clone(),
        RuntimeOwnership::External,
        RuntimeConsentState::ReuseApproved,
        RuntimeConsentState::NotRequested,
        7,
    )
    .expect("policy is valid");
    persistence
        .runtime_policy()
        .upsert(&policy)
        .expect("policy writes");

    let stored = persistence
        .runtime_policy()
        .get(&provider_id)
        .expect("policy reads")
        .expect("policy exists");

    // Existing rows carry no acknowledgement: the column is additive and nullable.
    assert_eq!(stored.acknowledged_untested_version, None);
}

#[test]
fn untested_version_acknowledgement_round_trips_and_can_be_cleared() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let provider_id = RuntimeProviderId::new("test.runtime");

    let acknowledged = RuntimePolicyRecord::new(
        provider_id.clone(),
        RuntimeOwnership::External,
        RuntimeConsentState::ReuseApproved,
        RuntimeConsentState::NotRequested,
        7,
    )
    .expect("policy is valid")
    .with_acknowledged_untested_version(Some("0.32.14".to_owned()))
    .expect("acknowledgement is valid");
    persistence
        .runtime_policy()
        .upsert(&acknowledged)
        .expect("policy writes");

    assert_eq!(
        persistence
            .runtime_policy()
            .get(&provider_id)
            .expect("policy reads")
            .expect("policy exists")
            .acknowledged_untested_version
            .as_deref(),
        Some("0.32.14")
    );

    let cleared = acknowledged
        .clone()
        .with_acknowledged_untested_version(None)
        .expect("clearing is valid");
    persistence
        .runtime_policy()
        .upsert(&cleared)
        .expect("policy rewrites");

    assert_eq!(
        persistence
            .runtime_policy()
            .get(&provider_id)
            .expect("policy reads")
            .expect("policy exists")
            .acknowledged_untested_version,
        None
    );
}

#[test]
fn an_oversized_acknowledged_version_is_rejected_before_sql() {
    let policy = RuntimePolicyRecord::new(
        RuntimeProviderId::new("test.runtime"),
        RuntimeOwnership::External,
        RuntimeConsentState::ReuseApproved,
        RuntimeConsentState::NotRequested,
        7,
    )
    .expect("policy is valid");

    assert!(
        policy
            .with_acknowledged_untested_version(Some("9".repeat(65)))
            .is_err()
    );
}
