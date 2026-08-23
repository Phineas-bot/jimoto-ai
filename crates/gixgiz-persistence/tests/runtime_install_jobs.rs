//! Durable runtime-installation storage behaviour.

use gixgiz_persistence::{
    CURRENT_SCHEMA_VERSION, DataRoot, PersistedRuntimeInstallJob, Persistence,
};

fn test_root(temporary: &tempfile::TempDir) -> DataRoot {
    DataRoot::from_override(temporary.path().join("gixgiz-test-root"))
        .expect("isolated test root initializes")
}

fn job(job_id: &str, state: &str) -> PersistedRuntimeInstallJob {
    PersistedRuntimeInstallJob {
        job_id: job_id.to_owned(),
        provider_id: "test.runtime".to_owned(),
        plan_revision: 1,
        version: "1.2.3".to_owned(),
        source_origin: "https://example.invalid".to_owned(),
        artifact_name: "TestRuntimeSetup.exe".to_owned(),
        expected_size_bytes: 1024,
        requires_administrator: false,
        state: state.to_owned(),
        stage: "awaiting_approval".to_owned(),
        approved_revision: None,
        approved_at_unix_ms: None,
        transferred_bytes: None,
        verification_json: None,
        effects_json: "{\"effects\":[]}".to_owned(),
        attention: Some("approval_required".to_owned()),
        failure: None,
        recovery: None,
        ownership: "unknown".to_owned(),
        retry_count: 0,
        created_at_unix_ms: 10,
        updated_at_unix_ms: 10,
    }
}

#[test]
fn migration_six_applies_and_reports_the_current_schema_version() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");

    // Install-job storage arrived in migration 6 and later migrations keep it.
    const { assert!(CURRENT_SCHEMA_VERSION >= 6) };
    assert_eq!(
        persistence
            .health_check()
            .expect("health check succeeds")
            .schema_version,
        CURRENT_SCHEMA_VERSION
    );
}

#[test]
fn an_install_job_round_trips_through_sqlite() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let repository = persistence.runtime_install();

    let record = job("job-1", "awaiting_approval");
    repository.upsert(&record).expect("job writes");

    let stored = repository
        .get("job-1")
        .expect("job reads")
        .expect("job exists");
    assert_eq!(stored, record);
}

#[test]
fn only_one_non_terminal_install_job_can_exist_per_provider() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let repository = persistence.runtime_install();

    repository
        .upsert(&job("job-1", "approved"))
        .expect("first job writes");

    // A second concurrently active job would allow an approved system change to
    // start twice, so the unique index must refuse it.
    assert!(repository.upsert(&job("job-2", "approved")).is_err());

    // Once the first job finishes, another may begin.
    let mut finished = job("job-1", "ready");
    finished.updated_at_unix_ms = 20;
    repository.upsert(&finished).expect("first job completes");
    repository
        .upsert(&job("job-2", "approved"))
        .expect("second job writes after the first finished");
}

#[test]
fn the_active_job_is_the_one_recovered_after_restart() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let repository = persistence.runtime_install();

    repository
        .upsert(&job("job-done", "ready"))
        .expect("terminal job writes");
    repository
        .upsert(&job("job-live", "running"))
        .expect("active job writes");

    let active = repository
        .active("test.runtime")
        .expect("active query succeeds")
        .expect("an interrupted job exists");
    assert_eq!(active.job_id, "job-live");

    // A provider with no interrupted work reports nothing to reconcile.
    assert!(
        repository
            .active("other.runtime")
            .expect("active query succeeds")
            .is_none()
    );
}

#[test]
fn invalid_records_are_rejected_before_reaching_sql() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    let repository = persistence.runtime_install();

    let mut empty_id = job("job-1", "approved");
    empty_id.job_id = String::new();
    assert!(repository.upsert(&empty_id).is_err());

    let mut zero_revision = job("job-2", "approved");
    zero_revision.plan_revision = 0;
    assert!(repository.upsert(&zero_revision).is_err());

    let mut oversized_origin = job("job-3", "approved");
    oversized_origin.source_origin = "h".repeat(513);
    assert!(repository.upsert(&oversized_origin).is_err());

    let mut embedded_nul = job("job-4", "approved");
    embedded_nul.artifact_name = "bad\0name.exe".to_owned();
    assert!(repository.upsert(&embedded_nul).is_err());
}

#[test]
fn install_storage_holds_no_binary_output_or_secret_columns() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");
    drop(persistence);

    let columns: Vec<String> =
        rusqlite::Connection::open(temporary.path().join("gixgiz-test-root/data/gixgiz.db"))
            .expect("database opens for inspection")
            .prepare("SELECT name FROM pragma_table_info('runtime_install_jobs')")
            .expect("column query prepares")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("columns enumerate")
            .collect::<Result<Vec<_>, _>>()
            .expect("columns decode");

    assert!(!columns.is_empty());
    for forbidden in [
        "installer_binary",
        "artifact_bytes",
        "process_output",
        "stdout",
        "stderr",
        "command_line",
        "arguments",
        "credential",
        "token",
        "secret",
        "install_path",
        "staging_path",
    ] {
        assert!(
            !columns.iter().any(|column| column.contains(forbidden)),
            "runtime_install_jobs must not store {forbidden}"
        );
    }
}
