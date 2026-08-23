use super::*;
use gixgiz_contracts::{
    AccelerationEvidence, AccelerationKind, ArchitectureEvidence, CorrelationId, CpuEvidence,
    EvidenceAvailability, EvidenceConfidence, EvidenceMetadata, EvidenceSource,
    GpuCollectionEvidence, GpuEvidence, MACHINE_PROFILE_SCHEMA_VERSION, MachineArchitecture,
    MachineProfile, MachineProfileCompleteness, OperatingSystemEvidence, OperationId,
    PhysicalMemoryEvidence, PreferencePriority, RecommendationReasonCode,
    RecommendationWarningCode, RuleSetVersion, StorageEvidence, StorageLocation,
    StorageMediaEvidence, StorageMediaKind, StringEvidence, U32Evidence, U64Evidence,
    UnknownReasonCode, UserPreferenceProfile, WorkloadTier,
};

const GIB: u64 = 1024 * 1024 * 1024;

#[test]
fn high_end_fixture_returns_recommended_fallback_and_larger_plan() {
    let report = engine()
        .recommend(&fixture(32, 20, 100, 8, true), balanced())
        .expect("high-end fixture recommends");

    assert_eq!(report.status, CapabilityReportStatus::PlansAvailable);
    assert_eq!(
        report
            .recommended_plan
            .as_ref()
            .map(|plan| plan.model.size_class),
        Some(gixgiz_contracts::ModelSizeClass::Standard)
    );
    assert!(report.fallback_plan.is_some());
    assert!(report.optional_larger_plan.is_some());
}

#[test]
fn medium_and_low_resource_fixtures_choose_conservative_plans() {
    let medium = engine()
        .recommend(&fixture(16, 10, 30, 4, false), balanced())
        .expect("medium fixture recommends");
    let low = engine()
        .recommend(&fixture(8, 4, 10, 2, false), balanced())
        .expect("low fixture recommends");

    assert_eq!(
        medium
            .recommended_plan
            .as_ref()
            .map(|plan| plan.model.size_class),
        Some(gixgiz_contracts::ModelSizeClass::Standard)
    );
    assert_eq!(
        low.recommended_plan
            .as_ref()
            .map(|plan| plan.model.size_class),
        Some(gixgiz_contracts::ModelSizeClass::Compact)
    );
    assert!(low.fallback_plan.is_none());
}

#[test]
fn cpu_only_fixture_never_infers_gpu_resources() {
    let report = engine()
        .recommend(&fixture(16, 10, 30, 4, false), balanced())
        .expect("CPU-only fixture recommends");
    let plan = report.recommended_plan.expect("plan exists");

    assert!(plan.resources.cpu_only);
    assert_eq!(plan.resources.gpu_memory_bytes, None);
    assert_eq!(plan.resources.acceleration, None);
    assert!(
        plan.warnings
            .iter()
            .any(|item| item.code == RecommendationWarningCode::CpuOnlyMode)
    );
}

#[test]
fn unknown_vram_and_acceleration_reduce_confidence_without_guessing() {
    let mut profile = fixture(16, 10, 30, 4, false);
    profile.gpus.devices.push(unknown_gpu());
    profile.acceleration = vec![AccelerationEvidence {
        kind: AccelerationKind::DirectMl,
        supported: None,
        metadata: unknown_metadata(),
    }];
    profile.completeness = MachineProfileCompleteness::Partial;

    let report = engine()
        .recommend(&profile, balanced())
        .expect("unknown optional evidence stays safe");
    assert_eq!(report.confidence, ConfidenceLevel::Medium);
    assert!(
        report
            .warnings
            .iter()
            .any(|item| { item.code == RecommendationWarningCode::VramUnknown })
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|item| { item.code == RecommendationWarningCode::AccelerationUnknown })
    );
    assert!(
        report
            .recommended_plan
            .as_ref()
            .is_some_and(|plan| plan.resources.cpu_only)
    );
}

#[test]
fn low_storage_returns_explicit_no_plan() {
    let report = engine()
        .recommend(&fixture(16, 10, 2, 4, false), balanced())
        .expect("low storage is a domain result");

    assert_eq!(report.status, CapabilityReportStatus::NoPlan);
    assert!(report.recommended_plan.is_none());
    assert!(
        report
            .reasons
            .iter()
            .any(|item| { item.code == RecommendationReasonCode::InsufficientStorage })
    );
}

#[test]
fn removable_storage_is_actionable_but_not_automatically_rejected() {
    let mut profile = fixture(16, 10, 30, 4, false);
    profile.storage.location = StorageLocation::UserSelected;
    profile.storage.media_kind.value = Some(StorageMediaKind::Removable);
    let report = engine()
        .recommend(&profile, balanced())
        .expect("removable storage with headroom recommends");

    assert_eq!(report.status, CapabilityReportStatus::PlansAvailable);
    assert!(
        report
            .warnings
            .iter()
            .any(|item| { item.code == RecommendationWarningCode::RemovableStorage })
    );
}

#[test]
fn hard_constraint_cannot_be_overridden_by_quality_preference() {
    let preferences = UserPreferenceProfile {
        workload: WorkloadTier::GeneralText,
        priority: PreferencePriority::BestQualityWithinSafeLimits,
        include_optional_larger: true,
    };
    let report = engine()
        .recommend(&fixture(8, 4, 10, 2, false), preferences)
        .expect("low fixture still returns a safe result");

    assert_eq!(
        report
            .recommended_plan
            .as_ref()
            .map(|plan| plan.model.size_class),
        Some(gixgiz_contracts::ModelSizeClass::Compact)
    );
}

#[test]
fn missing_critical_evidence_blocks_unsafe_plans_and_lowers_confidence() {
    let mut profile = fixture(16, 10, 30, 4, false);
    profile.physical_memory.total_bytes.value = None;
    profile.physical_memory.total_bytes.metadata = unknown_metadata();
    profile.completeness = MachineProfileCompleteness::Partial;
    let report = engine()
        .recommend(&profile, balanced())
        .expect("unknown memory returns no-plan");

    assert_eq!(report.status, CapabilityReportStatus::NoPlan);
    assert_eq!(report.confidence, ConfidenceLevel::Low);
    assert!(
        report
            .reasons
            .iter()
            .any(|item| { item.code == RecommendationReasonCode::CriticalEvidenceUnknown })
    );
}

#[test]
fn unknown_storage_location_blocks_planning_even_with_reported_free_space() {
    let mut profile = fixture(16, 10, 30, 4, false);
    profile.storage.location = StorageLocation::Unknown;

    let report = engine()
        .recommend(&profile, balanced())
        .expect("unknown storage is an explicit domain result");

    assert_eq!(report.status, CapabilityReportStatus::NoPlan);
    assert_eq!(report.confidence, ConfidenceLevel::Low);
    assert!(
        report
            .reasons
            .iter()
            .any(|item| item.code == RecommendationReasonCode::CriticalEvidenceUnknown)
    );
}

#[test]
fn fixed_inputs_produce_identical_reports_and_stable_ordering() {
    let engine = engine();
    let profile = fixture(32, 20, 100, 8, true);
    let first = engine
        .recommend(&profile, balanced())
        .expect("first report");
    let second = engine
        .recommend(&profile, balanced())
        .expect("second report");

    assert_eq!(first, second);
    assert_eq!(
        first.generated_from_scan_unix_ms,
        profile.scanned_at_unix_ms
    );
}

#[test]
fn equal_scores_use_stable_catalogue_identity_as_the_tie_breaker() {
    let mut engine = engine();
    let mut duplicate = engine.catalogue.candidates[1].clone();
    duplicate.model.catalogue_id =
        gixgiz_contracts::CandidateModelId::new("aaa-stable-tie-breaker");
    engine.catalogue.candidates.push(duplicate);

    let report = engine
        .recommend(&fixture(16, 10, 30, 4, false), balanced())
        .expect("tie remains deterministic");
    let plan = report.recommended_plan.expect("plan exists");

    assert_eq!(plan.model.catalogue_id.as_str(), "aaa-stable-tie-breaker");
    assert!(
        plan.reasons
            .iter()
            .any(|item| item.code == RecommendationReasonCode::StableCatalogueOrder)
    );
}

#[test]
fn catalogue_and_rule_versions_are_recorded_and_change_output() {
    let profile = fixture(16, 10, 30, 4, false);
    let first = engine()
        .recommend(&profile, balanced())
        .expect("default versions report");
    let mut changed = engine();
    changed.catalogue.version = gixgiz_contracts::CatalogueVersion::new("catalogue-test-v2");
    changed.rule_set_version = RuleSetVersion::new("rules-test-v2");
    let second = changed
        .recommend(&profile, balanced())
        .expect("changed versions report");

    assert_ne!(first, second);
    assert_eq!(second.catalogue_version.as_str(), "catalogue-test-v2");
    assert_eq!(second.rule_set_version.as_str(), "rules-test-v2");
    for plan in [
        second.recommended_plan.as_ref(),
        second.fallback_plan.as_ref(),
        second.optional_larger_plan.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        assert_eq!(plan.catalogue_version.as_str(), "catalogue-test-v2");
        assert_eq!(plan.rule_set_version.as_str(), "rules-test-v2");
    }

    let mut catalogue_only = engine();
    catalogue_only.catalogue.version = gixgiz_contracts::CatalogueVersion::new("catalogue-only-v2");
    let catalogue_report = catalogue_only
        .recommend(&profile, balanced())
        .expect("catalogue-only version report");
    assert_ne!(first, catalogue_report);

    let mut rules_only = engine();
    rules_only.rule_set_version = RuleSetVersion::new("rules-only-v2");
    let rules_report = rules_only
        .recommend(&profile, balanced())
        .expect("rules-only version report");
    assert_ne!(first, rules_report);
}

#[test]
fn coding_preference_keeps_every_plan_role_in_the_coding_family() {
    let report = engine()
        .recommend(
            &fixture(32, 24, 30, 8, false),
            UserPreferenceProfile {
                workload: WorkloadTier::Coding,
                ..balanced()
            },
        )
        .expect("coding fixture recommends");

    assert!(report.fallback_plan.is_some());
    assert!(report.optional_larger_plan.is_some());
    for plan in [
        report.recommended_plan.as_ref(),
        report.fallback_plan.as_ref(),
        report.optional_larger_plan.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        assert_eq!(plan.model.family, "Qwen 2.5 Coder");
        assert!(plan.model.workload_tiers.contains(&WorkloadTier::Coding));
    }
}

#[test]
fn unknown_preferences_are_rejected_with_typed_error() {
    let result = engine().recommend(
        &fixture(16, 10, 30, 4, false),
        UserPreferenceProfile {
            workload: WorkloadTier::Unknown,
            ..balanced()
        },
    );

    assert_eq!(result, Err(CoreError::InvalidRecommendationPreferences));
}

fn engine() -> CapabilityEngine {
    CapabilityEngine::v0_1()
}

fn balanced() -> UserPreferenceProfile {
    UserPreferenceProfile {
        workload: WorkloadTier::GeneralText,
        priority: PreferencePriority::Balanced,
        include_optional_larger: true,
    }
}

fn fixture(
    total_gib: u64,
    available_gib: u64,
    free_storage_gib: u64,
    logical_processors: u32,
    accelerated: bool,
) -> MachineProfile {
    let available = available_metadata();
    let text = |value: &str| StringEvidence {
        value: Some(value.to_owned()),
        metadata: available.clone(),
    };
    let number = |value| U64Evidence {
        value: Some(value),
        metadata: available.clone(),
    };
    MachineProfile {
        schema_version: MACHINE_PROFILE_SCHEMA_VERSION,
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
                metadata: available.clone(),
            },
        },
        cpu: CpuEvidence {
            name: text("Fixture CPU"),
            vendor: text("Fixture vendor"),
            physical_core_count: U32Evidence {
                value: Some(logical_processors.div_ceil(2)),
                metadata: available.clone(),
            },
            logical_core_count: U32Evidence {
                value: Some(logical_processors),
                metadata: available.clone(),
            },
        },
        physical_memory: PhysicalMemoryEvidence {
            total_bytes: number(total_gib * GIB),
            available_bytes: number(available_gib * GIB),
        },
        gpus: GpuCollectionEvidence {
            devices: accelerated
                .then(|| reliable_gpu(8 * GIB))
                .into_iter()
                .collect(),
            metadata: available.clone(),
        },
        acceleration: vec![AccelerationEvidence {
            kind: AccelerationKind::DirectMl,
            supported: Some(accelerated),
            metadata: available.clone(),
        }],
        storage: StorageEvidence {
            location: StorageLocation::ApplicationData,
            capacity_bytes: number(200 * GIB),
            free_bytes: number(free_storage_gib * GIB),
            filesystem: text("NTFS"),
            media_kind: StorageMediaEvidence {
                value: Some(StorageMediaKind::Fixed),
                metadata: available,
            },
        },
    }
}

fn reliable_gpu(memory: u64) -> GpuEvidence {
    let metadata = available_metadata();
    GpuEvidence {
        name: StringEvidence {
            value: Some("Fixture GPU".to_owned()),
            metadata: metadata.clone(),
        },
        vendor: StringEvidence {
            value: Some("Fixture vendor".to_owned()),
            metadata: metadata.clone(),
        },
        dedicated_memory_bytes: U64Evidence {
            value: Some(memory),
            metadata: metadata.clone(),
        },
        shared_memory_bytes: U64Evidence {
            value: Some(2 * GIB),
            metadata,
        },
    }
}

fn unknown_gpu() -> GpuEvidence {
    GpuEvidence {
        name: StringEvidence {
            value: Some("Unknown memory GPU".to_owned()),
            metadata: available_metadata(),
        },
        vendor: StringEvidence {
            value: Some("Fixture vendor".to_owned()),
            metadata: available_metadata(),
        },
        dedicated_memory_bytes: U64Evidence {
            value: None,
            metadata: unknown_metadata(),
        },
        shared_memory_bytes: U64Evidence {
            value: None,
            metadata: unknown_metadata(),
        },
    }
}

fn available_metadata() -> EvidenceMetadata {
    EvidenceMetadata::available(EvidenceSource::WindowsCim, EvidenceConfidence::High)
}

fn unknown_metadata() -> EvidenceMetadata {
    EvidenceMetadata::unavailable(
        EvidenceSource::WindowsCim,
        EvidenceAvailability::NotReliable,
        UnknownReasonCode::SourceUnreliable,
        "Fixture evidence is intentionally unknown.",
    )
}

/// Manual end-to-end diagnostic: real Windows scan into the real engine.
///
/// Prints exactly what the desktop would receive so a "no safe plan" result can
/// be traced to the specific missing evidence rather than guessed at.
#[test]
#[ignore = "manual Windows scan-to-recommendation diagnostic"]
fn manual_real_scan_produces_a_recommendation() {
    use crate::{CapabilityEngine, HardwareScanner, OperationContext};
    use gixgiz_contracts::{OperationId, PreferencePriority, UserPreferenceProfile, WorkloadTier};

    let scanner = HardwareScanner::windows().expect("windows scanner is available");
    let context = OperationContext::generated();
    let profile = scanner
        .scan(OperationId::new(), &context)
        .expect("scan succeeds");

    println!("completeness      : {:?}", profile.completeness);
    println!(
        "architecture      : {:?}  availability={:?} reason={:?}",
        profile.operating_system.architecture.value,
        profile.operating_system.architecture.metadata.availability,
        profile.operating_system.architecture.metadata.reason_code,
    );
    println!(
        "os name           : {:?}",
        profile.operating_system.name.value
    );
    println!(
        "total memory      : {:?}  availability={:?}",
        profile.physical_memory.total_bytes.value,
        profile.physical_memory.total_bytes.metadata.availability,
    );
    println!(
        "available memory  : {:?}  availability={:?}",
        profile.physical_memory.available_bytes.value,
        profile
            .physical_memory
            .available_bytes
            .metadata
            .availability,
    );
    println!(
        "cpu logical       : {:?}  physical={:?}",
        profile.cpu.logical_core_count.value, profile.cpu.physical_core_count.value
    );
    println!(
        "storage capacity  : {:?}  free={:?}  availability={:?}",
        profile.storage.capacity_bytes.value,
        profile.storage.free_bytes.value,
        profile.storage.free_bytes.metadata.availability,
    );

    let report = CapabilityEngine::v0_1()
        .recommend(
            &profile,
            UserPreferenceProfile {
                workload: WorkloadTier::GeneralText,
                priority: PreferencePriority::Balanced,
                include_optional_larger: true,
            },
        )
        .expect("recommendation succeeds");

    println!("\nreport status     : {:?}", report.status);
    for reason in &report.reasons {
        println!("  reason  {:?}: {}", reason.code, reason.message);
    }
    for warning in &report.warnings {
        println!("  warning {:?}: {}", warning.code, warning.message);
    }
    if let Some(plan) = report.recommended_plan.as_ref() {
        println!("recommended       : {}", plan.model.display_name);
    }
    if let Some(no_plan) = report.no_plan.as_ref() {
        for reason in &no_plan.reasons {
            println!("  no-plan {:?}: {}", reason.code, reason.message);
        }
    }
}
