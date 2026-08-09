use gixgiz_contracts::{
    AccelerationKind, CandidateModel, CandidateModelId, CandidateRuntime, CandidateRuntimeId,
    CatalogueVersion, MachineArchitecture, ModelSizeClass, WorkloadTier,
};

pub(super) const CATALOGUE_VERSION: &str = "gixgiz-catalogue-v0.1.0";
pub(super) const RULE_SET_VERSION: &str = "gixgiz-capability-rules-v0.1.0";

const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct CatalogueCandidate {
    pub(super) model: CandidateModel,
    pub(super) runtime: CandidateRuntime,
    pub(super) required_memory_bytes: u64,
    pub(super) memory_safety_margin_bytes: u64,
    pub(super) required_storage_bytes: u64,
    pub(super) storage_safety_margin_bytes: u64,
    pub(super) optional_gpu_memory_bytes: u64,
    pub(super) planned_context_tokens: u32,
    pub(super) minimum_logical_processors: u32,
    pub(super) quality_rank: u8,
    pub(super) requires_acceleration: bool,
}

#[derive(Clone)]
pub(super) struct CapabilityCatalogue {
    pub(super) version: CatalogueVersion,
    pub(super) candidates: Vec<CatalogueCandidate>,
}

pub(super) fn v0_1_catalogue() -> CapabilityCatalogue {
    let runtime = CandidateRuntime {
        catalogue_id: CandidateRuntimeId::new("gixgiz.local-text-runtime.v1"),
        display_name: "Local text runtime".to_owned(),
        supports_cpu_only: true,
        supported_architectures: vec![MachineArchitecture::X86_64],
        optional_accelerations: vec![
            AccelerationKind::DirectMl,
            AccelerationKind::Cuda,
            AccelerationKind::Rocm,
        ],
    };
    let mut candidates = Vec::with_capacity(6);
    candidates.extend(workload_candidates(
        &runtime,
        WorkloadTier::GeneralText,
        "qwen2.5",
        "Qwen 2.5",
        "Qwen/Qwen2.5",
    ));
    candidates.extend(workload_candidates(
        &runtime,
        WorkloadTier::Coding,
        "qwen2.5-coder",
        "Qwen 2.5 Coder",
        "Qwen/Qwen2.5-Coder",
    ));

    CapabilityCatalogue {
        version: CatalogueVersion::new(CATALOGUE_VERSION),
        candidates,
    }
}

fn workload_candidates(
    runtime: &CandidateRuntime,
    workload: WorkloadTier,
    id_family: &str,
    display_family: &str,
    provenance_family: &str,
) -> Vec<CatalogueCandidate> {
    [
        candidate(
            runtime,
            workload,
            CandidateDefinition {
                id: format!("{id_family}.0.5b-instruct"),
                display_name: format!("{display_family} Compact"),
                family: display_family.to_owned(),
                provenance_url: format!("https://huggingface.co/{provenance_family}-0.5B-Instruct"),
                size_class: ModelSizeClass::Compact,
                required_memory_bytes: 2 * GIB,
                memory_safety_margin_bytes: 2 * GIB,
                required_storage_bytes: GIB,
                storage_safety_margin_bytes: 2 * GIB,
                optional_gpu_memory_bytes: GIB,
                planned_context_tokens: 4096,
                minimum_logical_processors: 2,
                quality_rank: 1,
            },
        ),
        candidate(
            runtime,
            workload,
            CandidateDefinition {
                id: format!("{id_family}.1.5b-instruct"),
                display_name: format!("{display_family} Everyday"),
                family: display_family.to_owned(),
                provenance_url: format!("https://huggingface.co/{provenance_family}-1.5B-Instruct"),
                size_class: ModelSizeClass::Standard,
                required_memory_bytes: 4 * GIB,
                memory_safety_margin_bytes: 3 * GIB,
                required_storage_bytes: 2 * GIB,
                storage_safety_margin_bytes: 2 * GIB,
                optional_gpu_memory_bytes: 2 * GIB,
                planned_context_tokens: 8192,
                minimum_logical_processors: 4,
                quality_rank: 2,
            },
        ),
        candidate(
            runtime,
            workload,
            CandidateDefinition {
                id: format!("{id_family}.7b-instruct"),
                display_name: format!("{display_family} Quality"),
                family: display_family.to_owned(),
                provenance_url: format!("https://huggingface.co/{provenance_family}-7B-Instruct"),
                size_class: ModelSizeClass::Large,
                required_memory_bytes: 8 * GIB,
                memory_safety_margin_bytes: 4 * GIB,
                required_storage_bytes: 6 * GIB,
                storage_safety_margin_bytes: 3 * GIB,
                optional_gpu_memory_bytes: 5 * GIB,
                planned_context_tokens: 8192,
                minimum_logical_processors: 6,
                quality_rank: 3,
            },
        ),
    ]
    .into_iter()
    .collect()
}

struct CandidateDefinition {
    id: String,
    display_name: String,
    family: String,
    provenance_url: String,
    size_class: ModelSizeClass,
    required_memory_bytes: u64,
    memory_safety_margin_bytes: u64,
    required_storage_bytes: u64,
    storage_safety_margin_bytes: u64,
    optional_gpu_memory_bytes: u64,
    planned_context_tokens: u32,
    minimum_logical_processors: u32,
    quality_rank: u8,
}

fn candidate(
    runtime: &CandidateRuntime,
    workload: WorkloadTier,
    definition: CandidateDefinition,
) -> CatalogueCandidate {
    CatalogueCandidate {
        model: CandidateModel {
            catalogue_id: CandidateModelId::new(definition.id),
            display_name: definition.display_name,
            family: definition.family,
            workload_tiers: vec![workload],
            size_class: definition.size_class,
            licence_spdx: "Apache-2.0".to_owned(),
            provenance_url: definition.provenance_url,
        },
        runtime: runtime.clone(),
        required_memory_bytes: definition.required_memory_bytes,
        memory_safety_margin_bytes: definition.memory_safety_margin_bytes,
        required_storage_bytes: definition.required_storage_bytes,
        storage_safety_margin_bytes: definition.storage_safety_margin_bytes,
        optional_gpu_memory_bytes: definition.optional_gpu_memory_bytes,
        planned_context_tokens: definition.planned_context_tokens,
        minimum_logical_processors: definition.minimum_logical_processors,
        quality_rank: definition.quality_rank,
        requires_acceleration: false,
    }
}
