//! Deterministic capability filtering, scoring, and conservative estimation.

mod catalogue;

use std::cmp::Ordering;

use gixgiz_contracts::{
    AccelerationKind, CAPABILITY_REPORT_SCHEMA_VERSION, CapabilityReport, CapabilityReportStatus,
    CatalogueVersion, CompatibilityStatus, ConfidenceLevel, EvidenceAvailability,
    EvidenceConfidence, MACHINE_PROFILE_SCHEMA_VERSION, MachineArchitecture, MachineProfile,
    MachineProfileCompleteness, MemoryEstimate, NoPlanResult, PlanRole, PreferencePriority,
    RecommendationPlan, RecommendationReason, RecommendationReasonCode, RecommendationWarning,
    RecommendationWarningCode, ResourceEstimate, RuleSetVersion, StorageEstimate, StorageLocation,
    StorageMediaKind, UserPreferenceProfile, WorkloadTier,
};

use crate::CoreError;
use catalogue::{CapabilityCatalogue, CatalogueCandidate, RULE_SET_VERSION, v0_1_catalogue};

/// Deterministic provider-neutral capability recommendation service.
#[derive(Clone)]
pub struct CapabilityEngine {
    catalogue: CapabilityCatalogue,
    rule_set_version: RuleSetVersion,
}

impl CapabilityEngine {
    /// Creates the compiled v0.1 catalogue and rule set.
    #[must_use]
    pub fn v0_1() -> Self {
        Self {
            catalogue: v0_1_catalogue(),
            rule_set_version: RuleSetVersion::new(RULE_SET_VERSION),
        }
    }

    /// Generates a deterministic report from supplied evidence and preferences.
    pub fn recommend(
        &self,
        profile: &MachineProfile,
        preferences: UserPreferenceProfile,
    ) -> Result<CapabilityReport, CoreError> {
        if profile.schema_version != MACHINE_PROFILE_SCHEMA_VERSION {
            return Err(CoreError::UnsupportedMachineProfile {
                received: profile.schema_version,
            });
        }
        if matches!(preferences.workload, WorkloadTier::Unknown)
            || matches!(preferences.priority, PreferencePriority::Unknown)
        {
            return Err(CoreError::InvalidRecommendationPreferences);
        }

        let mut safe = Vec::new();
        let mut rejected_reasons = Vec::new();
        let mut rejected_unknown = false;
        for candidate in &self.catalogue.candidates {
            match evaluate_hard_constraints(profile, candidate) {
                ConstraintResult::Compatible(evaluation) => safe.push(ScoredCandidate {
                    score: score_candidate(candidate, preferences),
                    candidate: candidate.clone(),
                    evaluation,
                }),
                ConstraintResult::Rejected { reasons, unknown } => {
                    rejected_unknown |= unknown;
                    merge_reasons(&mut rejected_reasons, reasons);
                }
            }
        }

        safe.sort_by(|left, right| {
            right.score.cmp(&left.score).then_with(|| {
                left.candidate
                    .model
                    .catalogue_id
                    .cmp(&right.candidate.model.catalogue_id)
            })
        });

        if safe.is_empty() {
            return Ok(self.no_plan(profile, preferences, rejected_reasons, rejected_unknown));
        }

        let tie_broken = safe
            .get(1)
            .is_some_and(|second| second.score == safe[0].score);
        let recommended = safe[0].clone();
        let fallback = safe
            .iter()
            .filter(|item| {
                item.candidate
                    .model
                    .workload_tiers
                    .contains(&preferences.workload)
                    && item.candidate.quality_rank < recommended.candidate.quality_rank
            })
            .max_by(compare_quality_then_id)
            .cloned();
        let optional_larger = preferences
            .include_optional_larger
            .then(|| {
                safe.iter()
                    .filter(|item| {
                        item.candidate
                            .model
                            .workload_tiers
                            .contains(&preferences.workload)
                            && item.candidate.quality_rank > recommended.candidate.quality_rank
                    })
                    .min_by(compare_quality_then_id)
                    .cloned()
            })
            .flatten();

        let recommended_plan = build_plan(
            recommended.clone(),
            PlanRole::Recommended,
            preferences.priority,
            tie_broken,
            self.catalogue.version.clone(),
            self.rule_set_version.clone(),
        );
        let fallback_plan = fallback.map(|candidate| {
            build_plan(
                candidate,
                PlanRole::Fallback,
                preferences.priority,
                false,
                self.catalogue.version.clone(),
                self.rule_set_version.clone(),
            )
        });
        let optional_larger_plan = optional_larger.map(|candidate| {
            build_plan(
                candidate,
                PlanRole::OptionalLarger,
                preferences.priority,
                false,
                self.catalogue.version.clone(),
                self.rule_set_version.clone(),
            )
        });
        let mut warnings = recommended_plan.warnings.clone();
        if fallback_plan.is_none() {
            merge_warnings(
                &mut warnings,
                vec![warning(
                    RecommendationWarningCode::LimitedFallbackOptions,
                    "No distinct smaller plan passed every safety check.",
                )],
            );
        }
        if preferences.include_optional_larger && optional_larger_plan.is_none() {
            merge_warnings(
                &mut warnings,
                vec![warning(
                    RecommendationWarningCode::OptionalLargerUnavailable,
                    "No larger plan has enough verified resource headroom.",
                )],
            );
        }

        Ok(CapabilityReport {
            schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
            catalogue_version: self.catalogue.version.clone(),
            rule_set_version: self.rule_set_version.clone(),
            machine_profile_schema_version: profile.schema_version,
            generated_from_scan_unix_ms: profile.scanned_at_unix_ms,
            preferences,
            status: CapabilityReportStatus::PlansAvailable,
            fallback_plan,
            optional_larger_plan,
            no_plan: None,
            confidence: recommended_plan.confidence,
            reasons: recommended_plan.reasons.clone(),
            warnings,
            recommended_plan: Some(recommended_plan),
        })
    }

    fn no_plan(
        &self,
        profile: &MachineProfile,
        preferences: UserPreferenceProfile,
        mut reasons: Vec<RecommendationReason>,
        unknown: bool,
    ) -> CapabilityReport {
        reasons.insert(
            0,
            reason(
                RecommendationReasonCode::NoCompatibleCandidate,
                "No catalogue candidate passed every hard safety requirement.",
            ),
        );
        let mut warnings = profile_warnings(profile, None, None);
        merge_warnings(
            &mut warnings,
            vec![warning(
                RecommendationWarningCode::NoSafePlan,
                "GixGiz needs clearer evidence or more resource headroom before planning setup.",
            )],
        );
        let confidence = if unknown {
            ConfidenceLevel::Low
        } else {
            ConfidenceLevel::High
        };
        let no_plan = NoPlanResult {
            confidence,
            reasons: reasons.clone(),
            warnings: warnings.clone(),
        };

        CapabilityReport {
            schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
            catalogue_version: self.catalogue.version.clone(),
            rule_set_version: self.rule_set_version.clone(),
            machine_profile_schema_version: profile.schema_version,
            generated_from_scan_unix_ms: profile.scanned_at_unix_ms,
            preferences,
            status: CapabilityReportStatus::NoPlan,
            recommended_plan: None,
            fallback_plan: None,
            optional_larger_plan: None,
            no_plan: Some(no_plan),
            confidence,
            reasons,
            warnings,
        }
    }
}

impl Default for CapabilityEngine {
    fn default() -> Self {
        Self::v0_1()
    }
}

#[derive(Clone)]
struct CandidateEvaluation {
    resources: ResourceEstimate,
    confidence: ConfidenceLevel,
    reasons: Vec<RecommendationReason>,
    warnings: Vec<RecommendationWarning>,
}

#[derive(Clone)]
struct ScoredCandidate {
    candidate: CatalogueCandidate,
    evaluation: CandidateEvaluation,
    score: i32,
}

enum ConstraintResult {
    Compatible(CandidateEvaluation),
    Rejected {
        reasons: Vec<RecommendationReason>,
        unknown: bool,
    },
}

fn evaluate_hard_constraints(
    profile: &MachineProfile,
    candidate: &CatalogueCandidate,
) -> ConstraintResult {
    let mut rejected = Vec::new();
    let mut unknown = false;

    match reliable_value(
        profile.operating_system.architecture.value,
        &profile.operating_system.architecture.metadata,
    ) {
        Some(MachineArchitecture::Unknown) => {
            unknown = true;
            rejected.push(reason(
                RecommendationReasonCode::CriticalEvidenceUnknown,
                "Machine architecture evidence is required before planning setup.",
            ));
        }
        Some(architecture)
            if candidate
                .runtime
                .supported_architectures
                .contains(&architecture) => {}
        Some(_) => rejected.push(reason(
            RecommendationReasonCode::UnsupportedArchitecture,
            "The machine architecture is not supported by this runtime plan.",
        )),
        None => {
            unknown = true;
            rejected.push(reason(
                RecommendationReasonCode::CriticalEvidenceUnknown,
                "Machine architecture evidence is required before planning setup.",
            ));
        }
    }

    match reliable_value(
        profile.cpu.logical_core_count.value,
        &profile.cpu.logical_core_count.metadata,
    ) {
        Some(value) if value >= candidate.minimum_logical_processors => {}
        Some(_) => rejected.push(reason(
            RecommendationReasonCode::InsufficientLogicalProcessors,
            "The processor does not meet this plan's conservative minimum.",
        )),
        None => {
            unknown = true;
            rejected.push(reason(
                RecommendationReasonCode::CriticalEvidenceUnknown,
                "Logical processor evidence is required for a CPU-safe plan.",
            ));
        }
    }

    let total_memory = reliable_value(
        profile.physical_memory.total_bytes.value,
        &profile.physical_memory.total_bytes.metadata,
    );
    match total_memory {
        Some(value)
            if value
                >= candidate
                    .required_memory_bytes
                    .saturating_add(candidate.memory_safety_margin_bytes) => {}
        Some(_) => rejected.push(reason(
            RecommendationReasonCode::InsufficientMemory,
            "Total physical memory would not retain the required safety margin.",
        )),
        None => {
            unknown = true;
            rejected.push(reason(
                RecommendationReasonCode::CriticalEvidenceUnknown,
                "Total physical memory must be known before a safe plan can be selected.",
            ));
        }
    }

    let available_memory = reliable_value(
        profile.physical_memory.available_bytes.value,
        &profile.physical_memory.available_bytes.metadata,
    );
    if available_memory.is_some_and(|value| {
        value
            < candidate
                .required_memory_bytes
                .saturating_add(candidate.memory_safety_margin_bytes)
    }) {
        rejected.push(reason(
            RecommendationReasonCode::InsufficientAvailableMemory,
            "Currently available memory would not retain this plan's safety margin.",
        ));
    }

    if matches!(profile.storage.location, StorageLocation::Unknown) {
        unknown = true;
        rejected.push(reason(
            RecommendationReasonCode::CriticalEvidenceUnknown,
            "The selected storage location must be known before planning setup.",
        ));
    }

    let free_storage = reliable_value(
        profile.storage.free_bytes.value,
        &profile.storage.free_bytes.metadata,
    );
    match free_storage {
        Some(value)
            if value
                >= candidate
                    .required_storage_bytes
                    .saturating_add(candidate.storage_safety_margin_bytes) => {}
        Some(_) => rejected.push(reason(
            RecommendationReasonCode::InsufficientStorage,
            "Free storage would not retain the required safety margin.",
        )),
        None => {
            unknown = true;
            rejected.push(reason(
                RecommendationReasonCode::CriticalEvidenceUnknown,
                "Free storage must be known before a safe plan can be selected.",
            ));
        }
    }

    let acceleration = reliable_acceleration(profile, &candidate.runtime.optional_accelerations);
    let gpu_memory = reliable_gpu_memory(profile);
    let selected_acceleration = acceleration
        .filter(|_| gpu_memory.is_some_and(|bytes| bytes >= candidate.optional_gpu_memory_bytes));
    if candidate.requires_acceleration && selected_acceleration.is_none() {
        if acceleration.is_none() || gpu_memory.is_none() {
            unknown = true;
            rejected.push(reason(
                RecommendationReasonCode::CriticalEvidenceUnknown,
                "This plan requires acceleration and video memory that were not verified.",
            ));
        } else {
            rejected.push(reason(
                RecommendationReasonCode::InsufficientMemory,
                "Verified video memory is below this plan's requirement.",
            ));
        }
    } else if !candidate.runtime.supports_cpu_only && selected_acceleration.is_none() {
        rejected.push(reason(
            RecommendationReasonCode::UnsupportedArchitecture,
            "This runtime plan cannot operate in CPU-only mode.",
        ));
    }

    if !rejected.is_empty() {
        return ConstraintResult::Rejected {
            reasons: rejected,
            unknown,
        };
    }

    let mut confidence = ConfidenceLevel::High;
    let warnings = profile_warnings(profile, acceleration, gpu_memory);
    if profile.completeness != MachineProfileCompleteness::Complete
        || warnings.iter().any(|item| {
            matches!(
                item.code,
                RecommendationWarningCode::VramUnknown
                    | RecommendationWarningCode::AccelerationUnknown
            )
        })
    {
        confidence = reduce_confidence(confidence);
    }
    if available_memory.is_none() {
        confidence = ConfidenceLevel::Low;
    }

    let mut reasons = vec![
        reason(
            RecommendationReasonCode::SafeMemoryMargin,
            "Verified physical memory retains the catalogue safety margin.",
        ),
        reason(
            RecommendationReasonCode::SafeStorageMargin,
            "Verified free storage retains the catalogue safety margin.",
        ),
    ];
    if selected_acceleration.is_some() {
        reasons.push(reason(
            RecommendationReasonCode::ReliableAccelerationAvailable,
            "Reliable acceleration and video-memory evidence support optional GPU use.",
        ));
    } else {
        reasons.push(reason(
            RecommendationReasonCode::CpuOnlyFeasible,
            "This plan remains feasible without assuming GPU acceleration.",
        ));
    }

    ConstraintResult::Compatible(CandidateEvaluation {
        resources: ResourceEstimate {
            memory: MemoryEstimate {
                required_bytes: candidate.required_memory_bytes,
                safety_margin_bytes: candidate.memory_safety_margin_bytes,
                observed_total_bytes: total_memory,
                observed_available_bytes: available_memory,
            },
            storage: StorageEstimate {
                required_bytes: candidate.required_storage_bytes,
                safety_margin_bytes: candidate.storage_safety_margin_bytes,
                observed_free_bytes: free_storage,
            },
            planned_context_tokens: candidate.planned_context_tokens,
            cpu_only: selected_acceleration.is_none(),
            gpu_memory_bytes: selected_acceleration.and(gpu_memory),
            acceleration: selected_acceleration,
        },
        confidence,
        reasons,
        warnings,
    })
}

fn score_candidate(candidate: &CatalogueCandidate, preferences: UserPreferenceProfile) -> i32 {
    let workload_score = if candidate
        .model
        .workload_tiers
        .contains(&preferences.workload)
    {
        100
    } else {
        0
    };
    let size_score = match preferences.priority {
        PreferencePriority::Balanced => match candidate.quality_rank {
            2 => 30,
            3 => 20,
            _ => 10,
        },
        PreferencePriority::FastestSetup => 40 - i32::from(candidate.quality_rank) * 10,
        PreferencePriority::LowestResourceUse => 50 - i32::from(candidate.quality_rank) * 15,
        PreferencePriority::BestQualityWithinSafeLimits => i32::from(candidate.quality_rank) * 15,
        PreferencePriority::Unknown => 0,
        _ => 0,
    };
    workload_score + size_score
}

fn build_plan(
    candidate: ScoredCandidate,
    role: PlanRole,
    priority: PreferencePriority,
    tie_broken: bool,
    catalogue_version: CatalogueVersion,
    rule_set_version: RuleSetVersion,
) -> RecommendationPlan {
    let mut reasons = candidate.evaluation.reasons;
    reasons.insert(
        0,
        reason(
            RecommendationReasonCode::WorkloadMatch,
            "The catalogue entry matches the selected workload.",
        ),
    );
    reasons.push(match role {
        PlanRole::Fallback => reason(
            RecommendationReasonCode::SmallerFallback,
            "This smaller option preserves more resource headroom.",
        ),
        PlanRole::OptionalLarger => reason(
            RecommendationReasonCode::LargerAlternative,
            "This larger option is safe but uses more memory and storage.",
        ),
        _ => preference_reason(priority),
    });
    if tie_broken {
        reasons.push(reason(
            RecommendationReasonCode::StableCatalogueOrder,
            "Stable catalogue identity resolved an equal deterministic score.",
        ));
    }

    RecommendationPlan {
        catalogue_version,
        rule_set_version,
        role,
        compatibility: CompatibilityStatus::Compatible,
        model: candidate.candidate.model,
        runtime: candidate.candidate.runtime,
        resources: candidate.evaluation.resources,
        confidence: candidate.evaluation.confidence,
        reasons,
        warnings: candidate.evaluation.warnings,
    }
}

fn preference_reason(priority: PreferencePriority) -> RecommendationReason {
    match priority {
        PreferencePriority::Balanced => reason(
            RecommendationReasonCode::BalancedChoice,
            "This option balances capability with resource headroom.",
        ),
        PreferencePriority::FastestSetup => reason(
            RecommendationReasonCode::FastestSetup,
            "This option has the smallest safe setup footprint.",
        ),
        PreferencePriority::LowestResourceUse => reason(
            RecommendationReasonCode::LowestResourceUse,
            "This option preserves the most verified memory and storage headroom.",
        ),
        PreferencePriority::BestQualityWithinSafeLimits => reason(
            RecommendationReasonCode::BestQualityWithinSafeLimits,
            "This is the strongest catalogue option within verified safety limits.",
        ),
        PreferencePriority::Unknown => reason(
            RecommendationReasonCode::Unknown,
            "The preference could not be interpreted.",
        ),
        _ => reason(
            RecommendationReasonCode::Unknown,
            "The preference could not be interpreted.",
        ),
    }
}

fn profile_warnings(
    profile: &MachineProfile,
    acceleration: Option<AccelerationKind>,
    gpu_memory: Option<u64>,
) -> Vec<RecommendationWarning> {
    let mut warnings = Vec::new();
    if profile.completeness != MachineProfileCompleteness::Complete {
        warnings.push(warning(
            RecommendationWarningCode::PartialHardwareEvidence,
            "Some hardware evidence remains explicitly unavailable.",
        ));
    }
    if reliable_value(
        profile.physical_memory.available_bytes.value,
        &profile.physical_memory.available_bytes.metadata,
    )
    .is_none()
    {
        warnings.push(warning(
            RecommendationWarningCode::AvailableMemoryUnknown,
            "Currently available memory could not be verified; close other apps before setup.",
        ));
    }
    if !profile.gpus.devices.is_empty() && gpu_memory.is_none() {
        warnings.push(warning(
            RecommendationWarningCode::VramUnknown,
            "Video memory is unknown and is not counted toward this plan.",
        ));
    }
    if acceleration.is_none()
        && profile.acceleration.iter().any(|item| {
            item.supported.is_none()
                || item.metadata.availability != EvidenceAvailability::Available
        })
    {
        warnings.push(warning(
            RecommendationWarningCode::AccelerationUnknown,
            "Graphics acceleration is unknown and is not assumed available.",
        ));
    }
    if acceleration.is_none() {
        warnings.push(warning(
            RecommendationWarningCode::CpuOnlyMode,
            "The estimate uses a conservative processor-only plan.",
        ));
    }
    if profile.storage.media_kind.value == Some(StorageMediaKind::Removable) {
        warnings.push(warning(
            RecommendationWarningCode::RemovableStorage,
            "Keep the selected removable storage connected while local AI is in use.",
        ));
    }
    warnings
}

fn reliable_acceleration(
    profile: &MachineProfile,
    allowed: &[AccelerationKind],
) -> Option<AccelerationKind> {
    profile.acceleration.iter().find_map(|item| {
        (item.supported == Some(true)
            && item.metadata.availability == EvidenceAvailability::Available
            && matches!(
                item.metadata.confidence,
                EvidenceConfidence::High | EvidenceConfidence::Medium
            )
            && allowed.contains(&item.kind))
        .then_some(item.kind)
    })
}

fn reliable_gpu_memory(profile: &MachineProfile) -> Option<u64> {
    profile
        .gpus
        .devices
        .iter()
        .filter_map(|gpu| {
            reliable_value(
                gpu.dedicated_memory_bytes.value,
                &gpu.dedicated_memory_bytes.metadata,
            )
        })
        .max()
}

fn reliable_value<T: Copy>(
    value: Option<T>,
    metadata: &gixgiz_contracts::EvidenceMetadata,
) -> Option<T> {
    (metadata.availability == EvidenceAvailability::Available
        && matches!(
            metadata.confidence,
            EvidenceConfidence::High | EvidenceConfidence::Medium
        ))
    .then_some(value)
    .flatten()
}

fn compare_quality_then_id(left: &&ScoredCandidate, right: &&ScoredCandidate) -> Ordering {
    left.candidate
        .quality_rank
        .cmp(&right.candidate.quality_rank)
        .then_with(|| {
            right
                .candidate
                .model
                .catalogue_id
                .cmp(&left.candidate.model.catalogue_id)
        })
}

fn reduce_confidence(confidence: ConfidenceLevel) -> ConfidenceLevel {
    match confidence {
        ConfidenceLevel::High => ConfidenceLevel::Medium,
        ConfidenceLevel::Medium | ConfidenceLevel::Low | ConfidenceLevel::Unknown => {
            ConfidenceLevel::Low
        }
        _ => ConfidenceLevel::Low,
    }
}

fn reason(code: RecommendationReasonCode, message: &str) -> RecommendationReason {
    RecommendationReason {
        code,
        message: message.to_owned(),
    }
}

fn warning(code: RecommendationWarningCode, message: &str) -> RecommendationWarning {
    RecommendationWarning {
        code,
        message: message.to_owned(),
    }
}

fn merge_reasons(target: &mut Vec<RecommendationReason>, additions: Vec<RecommendationReason>) {
    for item in additions {
        if !target.iter().any(|existing| existing.code == item.code) {
            target.push(item);
        }
    }
}

fn merge_warnings(target: &mut Vec<RecommendationWarning>, additions: Vec<RecommendationWarning>) {
    for item in additions {
        if !target.iter().any(|existing| existing.code == item.code) {
            target.push(item);
        }
    }
}

#[cfg(test)]
mod tests;
