//! Storage-location validation and persistence behaviour.

use std::sync::Arc;

use gixgiz_contracts::{
    SetupDestinationCategory, StorageLocationKind, StorageLocationRejection,
    StorageLocationWarningCode,
};
use gixgiz_persistence::{DataRoot, Persistence};

use super::{FreeSpaceProbe, StorageLocationService};

struct FixedSpace(Option<u64>);

impl FreeSpaceProbe for FixedSpace {
    fn free_bytes(&self, _path: &std::path::Path) -> Option<u64> {
        self.0
    }
}

fn service(
    temporary: &tempfile::TempDir,
    free: Option<u64>,
) -> (Persistence, StorageLocationService) {
    let root = DataRoot::from_override(temporary.path().join("gixgiz-test-root"))
        .expect("isolated test root initializes");
    let persistence = Persistence::open(root).expect("database opens");
    let service =
        StorageLocationService::with_persistence(&persistence, Arc::new(FixedSpace(free)));
    (persistence, service)
}

fn plenty() -> u64 {
    64 * 1024 * 1024 * 1024
}

#[test]
fn a_network_or_device_path_is_never_a_storage_root() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));

    for (path, expected) in [
        (
            "\\\\server\\share\\models",
            StorageLocationRejection::NetworkPath,
        ),
        (
            "//server/share/models",
            StorageLocationRejection::NetworkPath,
        ),
        ("\\\\.\\C:\\models", StorageLocationRejection::NetworkPath),
        ("\\\\?\\C:\\models", StorageLocationRejection::NetworkPath),
    ] {
        let validation = service.validate(StorageLocationKind::Models, path);
        assert!(!validation.accepted, "{path} must be refused");
        assert_eq!(validation.rejection, Some(expected), "{path}");
    }
}

#[test]
fn a_relative_or_traversing_path_is_refused() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));

    let relative = service.validate(StorageLocationKind::Models, "models");
    assert_eq!(
        relative.rejection,
        Some(StorageLocationRejection::NotAbsolute)
    );

    let traversal = service.validate(StorageLocationKind::Models, r"C:\models\..\..\windows");
    assert_eq!(
        traversal.rejection,
        Some(StorageLocationRejection::InvalidPath)
    );
}

#[test]
fn an_empty_or_oversized_path_is_refused() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));

    assert_eq!(
        service
            .validate(StorageLocationKind::Models, "   ")
            .rejection,
        Some(StorageLocationRejection::InvalidPath)
    );
    let oversized = format!(r"C:\{}", "a".repeat(400));
    assert_eq!(
        service
            .validate(StorageLocationKind::Models, &oversized)
            .rejection,
        Some(StorageLocationRejection::InvalidPath)
    );
}

#[test]
fn a_drive_without_room_is_refused_before_anything_is_stored() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(1024));

    let validation = service.validate(
        StorageLocationKind::Models,
        &temporary.path().to_string_lossy(),
    );

    assert!(!validation.accepted);
    assert_eq!(
        validation.rejection,
        Some(StorageLocationRejection::InsufficientSpace)
    );
    assert_eq!(validation.free_bytes, Some(1024));
    assert_eq!(
        validation.required_bytes,
        StorageLocationKind::Models.required_bytes()
    );
}

#[test]
fn an_uninspectable_drive_is_refused() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, None);

    assert_eq!(
        service
            .validate(
                StorageLocationKind::Models,
                &temporary.path().to_string_lossy()
            )
            .rejection,
        Some(StorageLocationRejection::Unavailable)
    );
}

#[test]
fn an_accepted_location_round_trips_and_resolves() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));
    let chosen = temporary.path().join("models");
    std::fs::create_dir_all(&chosen).expect("chosen directory exists");

    let outcome = service
        .set(
            StorageLocationKind::Models,
            Some(&chosen.to_string_lossy()),
            false,
            false,
            10,
        )
        .expect("set succeeds");

    let setting = outcome.setting.expect("a setting was stored");
    assert_eq!(setting.category, SetupDestinationCategory::UserSelected);
    assert!(!setting.is_default);
    assert_eq!(
        service
            .resolved(StorageLocationKind::Models)
            .expect("resolves"),
        Some(chosen)
    );
}

#[test]
fn changing_a_location_always_warns_that_existing_content_stays() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));
    let chosen = temporary.path().join("models");
    std::fs::create_dir_all(&chosen).expect("chosen directory exists");

    let outcome = service
        .set(
            StorageLocationKind::Models,
            Some(&chosen.to_string_lossy()),
            false,
            false,
            10,
        )
        .expect("set succeeds");

    // Content is never moved, so the user must be told rather than discovering
    // models split across two places.
    assert!(
        outcome
            .warnings
            .iter()
            .any(|warning| warning.code == StorageLocationWarningCode::ExistingContentRemains)
    );
}

#[test]
fn clearing_a_location_restores_the_default() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));
    let chosen = temporary.path().join("models");
    std::fs::create_dir_all(&chosen).expect("chosen directory exists");
    service
        .set(
            StorageLocationKind::Models,
            Some(&chosen.to_string_lossy()),
            false,
            false,
            10,
        )
        .expect("set succeeds");

    let cleared = service
        .set(StorageLocationKind::Models, None, false, false, 20)
        .expect("clear succeeds");

    assert!(cleared.setting.expect("default setting").is_default);
    assert_eq!(
        service
            .resolved(StorageLocationKind::Models)
            .expect("resolves"),
        None
    );
}

#[test]
fn an_external_runtime_model_location_needs_explicit_acceptance() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));
    let chosen = temporary.path().join("models");
    std::fs::create_dir_all(&chosen).expect("chosen directory exists");

    // Without acceptance the runtime is never reconfigured.
    let refused = service
        .set(
            StorageLocationKind::Models,
            Some(&chosen.to_string_lossy()),
            true,
            false,
            10,
        )
        .expect("set completes");
    assert!(refused.setting.is_none());
    assert!(!refused.validation.accepted);
    assert_eq!(
        service
            .resolved(StorageLocationKind::Models)
            .expect("resolves"),
        None
    );

    // With acceptance the change is recorded and disclosed.
    let accepted = service
        .set(
            StorageLocationKind::Models,
            Some(&chosen.to_string_lossy()),
            true,
            true,
            20,
        )
        .expect("set succeeds");
    assert!(accepted.setting.is_some());
    assert!(
        accepted
            .warnings
            .iter()
            .any(|warning| warning.code == StorageLocationWarningCode::ExternalRuntimeReconfigured)
    );
}

#[test]
fn a_runtime_location_never_needs_external_consent() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));
    let chosen = temporary.path().join("runtime");
    std::fs::create_dir_all(&chosen).expect("chosen directory exists");

    // Installing a GixGiz-managed runtime elsewhere changes nothing the user
    // already owns, so it carries no ownership transition.
    let outcome = service
        .set(
            StorageLocationKind::Runtime,
            Some(&chosen.to_string_lossy()),
            true,
            false,
            10,
        )
        .expect("set succeeds");

    assert!(outcome.setting.is_some());
}

#[test]
fn settings_report_defaults_until_a_choice_is_made() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));

    let settings = service.settings(true).expect("settings read");

    assert_eq!(settings.len(), 3);
    assert!(settings.iter().all(|setting| setting.is_default));
    // Only model storage on an externally owned runtime needs extra consent.
    let models = settings
        .iter()
        .find(|setting| setting.kind == StorageLocationKind::Models)
        .expect("models setting");
    assert!(models.requires_external_consent);
    let runtime = settings
        .iter()
        .find(|setting| setting.kind == StorageLocationKind::Runtime)
        .expect("runtime setting");
    assert!(!runtime.requires_external_consent);
}

#[test]
fn an_unknown_kind_is_refused() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (_persistence, service) = service(&temporary, Some(plenty()));

    let outcome = service
        .set(
            StorageLocationKind::Unknown,
            Some(r"C:\somewhere"),
            false,
            false,
            10,
        )
        .expect("set completes");

    assert!(outcome.setting.is_none());
    assert!(!outcome.validation.accepted);
}
