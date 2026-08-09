use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AccelerationKind, CorrelationId, MachineArchitecture, MachineProfile, RequestId};

/// Schema generation for the first deterministic capability report.
pub const CAPABILITY_REPORT_SCHEMA_VERSION: u32 = 1;

macro_rules! bounded_string_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(
            Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Creates an identifier from repository-owned bounded text.
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Returns the identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

bounded_string_id!(
    CatalogueVersion,
    "Version of the deterministic local planning catalogue."
);
bounded_string_id!(
    RuleSetVersion,
    "Version of the deterministic hard-constraint and scoring rules."
);
bounded_string_id!(
    CandidateModelId,
    "Provider-neutral catalogue model identifier."
);
bounded_string_id!(
    CandidateRuntimeId,
    "Provider-neutral catalogue runtime identifier."
);

/// Beginner-facing workload selected by the user.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum WorkloadTier {
    /// Everyday writing, questions, summaries, and general text.
    GeneralText,
    /// Code explanation, drafting, and software-development assistance.
    Coding,
    /// A newer peer supplied an unrecognized workload.
    #[serde(other)]
    Unknown,
}

/// User priority applied only after hard compatibility filtering.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum PreferencePriority {
    /// Balance quality, setup size, and resource headroom.
    Balanced,
    /// Prefer the smallest setup likely to become usable quickly.
    FastestSetup,
    /// Preserve the most memory and storage headroom.
    LowestResourceUse,
    /// Prefer the strongest safe candidate supported by current evidence.
    BestQualityWithinSafeLimits,
    /// A newer peer supplied an unrecognized priority.
    #[serde(other)]
    Unknown,
}

/// Provider-neutral user preferences consumed by the Rust recommendation engine.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct UserPreferenceProfile {
    /// Primary intended workload.
    pub workload: WorkloadTier,
    /// Resource-versus-quality priority.
    pub priority: PreferencePriority,
    /// Whether a safe larger alternative may be returned for comparison.
    pub include_optional_larger: bool,
}

/// Coarse model-size category used for deterministic planning and explanation.
#[derive(
    Clone, Copy, Debug, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ModelSizeClass {
    /// Smallest catalogue tier with the lowest resource use.
    Compact,
    /// Middle catalogue tier balancing capability and headroom.
    Standard,
    /// Larger catalogue tier requiring substantially more resources.
    Large,
    /// A newer peer supplied an unrecognized size class.
    #[serde(other)]
    Unknown,
}

/// Provider-neutral model metadata selected from the local catalogue.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CandidateModel {
    /// Stable catalogue identity independent from provider install tags.
    pub catalogue_id: CandidateModelId,
    /// Beginner-readable model name.
    pub display_name: String,
    /// Canonical model family.
    pub family: String,
    /// Workloads explicitly represented by this catalogue entry.
    pub workload_tiers: Vec<WorkloadTier>,
    /// Coarse resource tier.
    pub size_class: ModelSizeClass,
    /// SPDX licence identifier recorded by the upstream model card.
    pub licence_spdx: String,
    /// Official upstream model-card URL used as provenance.
    pub provenance_url: String,
}

/// Provider-neutral runtime metadata required to explain a recommendation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CandidateRuntime {
    /// Stable runtime catalogue identity independent from provider product names.
    pub catalogue_id: CandidateRuntimeId,
    /// Beginner-readable runtime role.
    pub display_name: String,
    /// Whether the planned configuration can run without GPU acceleration.
    pub supports_cpu_only: bool,
    /// Architectures supported by this planning profile.
    pub supported_architectures: Vec<MachineArchitecture>,
    /// Acceleration kinds that may be used only when reliably evidenced.
    pub optional_accelerations: Vec<AccelerationKind>,
}

/// Compatibility result after hard constraints are evaluated.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    /// Every hard requirement is satisfied by available evidence.
    Compatible,
    /// At least one hard requirement is violated.
    Incompatible,
    /// Critical evidence is unavailable, so compatibility cannot be established safely.
    #[serde(other)]
    Unknown,
}

/// Confidence in a recommendation or no-plan conclusion.
#[derive(
    Clone, Copy, Debug, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    /// Critical evidence is reliable and optional uncertainty is limited.
    High,
    /// The plan is safe but one or more optional facts remain unknown.
    Medium,
    /// Material uncertainty limits the strength of the result.
    Low,
    /// Confidence was not established or a newer peer supplied an unknown value.
    #[serde(other)]
    Unknown,
}

/// Conservative memory estimate and the observed evidence used to assess it.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct MemoryEstimate {
    /// Estimated memory required by the planned local workload.
    pub required_bytes: u64,
    /// Additional memory retained as a safety margin.
    pub safety_margin_bytes: u64,
    /// Observed total physical memory when reliably available.
    pub observed_total_bytes: Option<u64>,
    /// Observed currently available physical memory when reliably available.
    pub observed_available_bytes: Option<u64>,
}

/// Conservative storage estimate and the observed evidence used to assess it.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StorageEstimate {
    /// Estimated installed model and runtime footprint.
    pub required_bytes: u64,
    /// Additional free space retained as a safety margin.
    pub safety_margin_bytes: u64,
    /// Observed currently free bytes at the selected storage location.
    pub observed_free_bytes: Option<u64>,
}

/// Conservative resource estimate for one candidate plan.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ResourceEstimate {
    /// Physical-memory estimate.
    pub memory: MemoryEstimate,
    /// Storage estimate.
    pub storage: StorageEstimate,
    /// Conservative context size used for planning, not a performance promise.
    pub planned_context_tokens: u32,
    /// Whether the plan remains feasible without GPU acceleration.
    pub cpu_only: bool,
    /// Reliably evidenced GPU memory assigned to the plan, if any.
    pub gpu_memory_bytes: Option<u64>,
    /// Reliably evidenced acceleration selected for the plan, if any.
    pub acceleration: Option<AccelerationKind>,
}

/// Role of one plan in the recommendation response.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum PlanRole {
    /// Primary safe plan selected by deterministic scoring.
    Recommended,
    /// Smaller plan that preserves more resource headroom.
    Fallback,
    /// Safe larger plan shown as an optional trade-off.
    OptionalLarger,
    /// A newer peer supplied an unrecognized role.
    #[serde(other)]
    Unknown,
}

/// Stable explanation category for deterministic selection decisions.
#[derive(
    Clone, Copy, Debug, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RecommendationReasonCode {
    /// Candidate matches the requested workload.
    WorkloadMatch,
    /// Candidate best balances resource use and capability.
    BalancedChoice,
    /// Candidate best matches the fastest-setup preference.
    FastestSetup,
    /// Candidate best preserves memory and storage.
    LowestResourceUse,
    /// Candidate is the strongest option within verified safety limits.
    BestQualityWithinSafeLimits,
    /// Verified physical-memory margin is sufficient.
    SafeMemoryMargin,
    /// Verified storage margin is sufficient.
    SafeStorageMargin,
    /// Candidate remains feasible without GPU acceleration.
    CpuOnlyFeasible,
    /// Reliable optional acceleration evidence can be used.
    ReliableAccelerationAvailable,
    /// Plan is the smaller fallback to the recommended plan.
    SmallerFallback,
    /// Plan is the larger optional alternative.
    LargerAlternative,
    /// Stable catalogue identity resolved an otherwise equal score.
    StableCatalogueOrder,
    /// No catalogue candidate passed every hard constraint.
    NoCompatibleCandidate,
    /// Machine architecture violates the runtime requirement.
    UnsupportedArchitecture,
    /// Logical processor evidence violates the minimum requirement.
    InsufficientLogicalProcessors,
    /// Physical memory violates the conservative requirement.
    InsufficientMemory,
    /// Currently available memory violates the conservative requirement.
    InsufficientAvailableMemory,
    /// Free storage violates the conservative requirement.
    InsufficientStorage,
    /// Critical evidence is unknown, so a safe plan cannot be established.
    CriticalEvidenceUnknown,
    /// A newer peer supplied an unrecognized reason.
    #[serde(other)]
    Unknown,
}

/// One stable reason plus plain-language explanation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RecommendationReason {
    /// Machine-readable reason category.
    pub code: RecommendationReasonCode,
    /// Safe provider-neutral explanation.
    pub message: String,
}

/// Stable warning category for uncertainty or a resource trade-off.
#[derive(
    Clone, Copy, Debug, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RecommendationWarningCode {
    /// The machine profile contains explicitly unavailable evidence.
    PartialHardwareEvidence,
    /// Currently available memory could not be verified.
    AvailableMemoryUnknown,
    /// Dedicated video memory could not be verified.
    VramUnknown,
    /// Supported acceleration could not be verified.
    AccelerationUnknown,
    /// The plan is intentionally conservative and CPU-only.
    CpuOnlyMode,
    /// The selected storage location is removable.
    RemovableStorage,
    /// No distinct smaller safe fallback is available.
    LimitedFallbackOptions,
    /// No larger safe alternative is available.
    OptionalLargerUnavailable,
    /// No safe plan can be recommended from current evidence.
    NoSafePlan,
    /// A newer peer supplied an unrecognized warning.
    #[serde(other)]
    Unknown,
}

/// One stable warning plus plain-language explanation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RecommendationWarning {
    /// Machine-readable warning category.
    pub code: RecommendationWarningCode,
    /// Safe provider-neutral explanation.
    pub message: String,
}

/// One explainable candidate plan returned by the recommendation engine.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RecommendationPlan {
    /// Catalogue version that defined this candidate.
    pub catalogue_version: CatalogueVersion,
    /// Rule-set version that selected this plan.
    pub rule_set_version: RuleSetVersion,
    /// Role in the report.
    pub role: PlanRole,
    /// Hard-constraint compatibility result.
    pub compatibility: CompatibilityStatus,
    /// Provider-neutral selected model.
    pub model: CandidateModel,
    /// Provider-neutral selected runtime planning profile.
    pub runtime: CandidateRuntime,
    /// Conservative resource estimates.
    pub resources: ResourceEstimate,
    /// Confidence in this plan.
    pub confidence: ConfidenceLevel,
    /// Deterministic explanations for this plan.
    pub reasons: Vec<RecommendationReason>,
    /// Uncertainty and trade-off warnings for this plan.
    pub warnings: Vec<RecommendationWarning>,
}

/// Whether the report contains a safe plan or an explicit no-plan result.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum CapabilityReportStatus {
    /// At least one safe plan is available.
    PlansAvailable,
    /// No safe plan can be established from the supplied evidence.
    NoPlan,
    /// A newer peer supplied an unrecognized report status.
    #[serde(other)]
    Unknown,
}

/// Explicit result when no catalogue candidate can be recommended safely.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct NoPlanResult {
    /// Confidence in the no-plan conclusion.
    pub confidence: ConfidenceLevel,
    /// Hard-constraint reasons that prevented a safe plan.
    pub reasons: Vec<RecommendationReason>,
    /// Actionable uncertainty or resource warnings.
    pub warnings: Vec<RecommendationWarning>,
}

/// Deterministic capability report generated from one machine profile.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CapabilityReport {
    /// Capability-report schema generation.
    pub schema_version: u32,
    /// Version of the local static catalogue used for this report.
    pub catalogue_version: CatalogueVersion,
    /// Version of the deterministic rule set used for this report.
    pub rule_set_version: RuleSetVersion,
    /// Schema version of the supplied machine profile.
    pub machine_profile_schema_version: u32,
    /// Source scan timestamp used as deterministic generation provenance.
    pub generated_from_scan_unix_ms: u64,
    /// Exact user preferences used for selection.
    pub preferences: UserPreferenceProfile,
    /// Whether safe plans are available.
    pub status: CapabilityReportStatus,
    /// Primary recommended plan, when available.
    pub recommended_plan: Option<RecommendationPlan>,
    /// Smaller safe fallback, when one exists.
    pub fallback_plan: Option<RecommendationPlan>,
    /// Safe larger alternative, when requested and available.
    pub optional_larger_plan: Option<RecommendationPlan>,
    /// Explicit no-plan detail when no safe plan exists.
    pub no_plan: Option<NoPlanResult>,
    /// Overall report confidence.
    pub confidence: ConfidenceLevel,
    /// Report-level deterministic reasons.
    pub reasons: Vec<RecommendationReason>,
    /// Report-level warnings and unknown-evidence impact.
    pub warnings: Vec<RecommendationWarning>,
}

/// Authenticated request for one deterministic capability report.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RecommendationRequest {
    /// Versioned machine evidence produced by Task 07.
    pub machine_profile: MachineProfile,
    /// User intent applied only after hard filtering.
    pub preferences: UserPreferenceProfile,
    /// Identifier shared by the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this recommendation request.
    pub request_id: RequestId,
}

/// Authenticated response containing one deterministic capability report.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RecommendationResponse {
    /// Deterministic capability report.
    pub report: CapabilityReport,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_contract_values_are_stable_and_forward_compatible() {
        assert_eq!(CAPABILITY_REPORT_SCHEMA_VERSION, 1);
        let role: PlanRole =
            serde_json::from_str("\"future_role\"").expect("unknown role is accepted");
        let warning: RecommendationWarningCode =
            serde_json::from_str("\"future_warning\"").expect("unknown warning is accepted");
        let compatibility: CompatibilityStatus =
            serde_json::from_str("\"future_status\"").expect("unknown compatibility is accepted");

        assert_eq!(role, PlanRole::Unknown);
        assert_eq!(warning, RecommendationWarningCode::Unknown);
        assert_eq!(compatibility, CompatibilityStatus::Unknown);
        assert_eq!(
            serde_json::to_string(&PreferencePriority::BestQualityWithinSafeLimits)
                .expect("priority serializes"),
            "\"best_quality_within_safe_limits\""
        );
    }
}
