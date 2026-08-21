//! Rust-owned SQLite persistence for GixGiz platform metadata.
//!
//! This crate owns data-root resolution, the single SQLite connection,
//! migrations, backup policy, health checks, and typed repositories. It does
//! not expose raw SQL or database handles to Flutter, transport clients, packs,
//! or higher-level application code. The schema intentionally excludes
//! secrets, private content, logs, artifacts, model binaries, and large blobs.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod chat;
mod data_root;
mod database;
mod error;
mod migrations;
mod repositories;
mod setup;

pub use chat::{
    ChatRepository, INTERRUPTED_FAILURE_CODE, PersistedChatMessage, PersistedChatMessageStatus,
    PersistedChatRole, PersistedConversation, PersistedConversationInput,
    PersistedGenerationAdmission, PersistedGenerationOutcome,
};
pub use data_root::DataRoot;
pub use database::{DatabaseConfiguration, Persistence, PersistenceHealth, PersistenceOptions};
pub use error::PersistenceError;
pub use migrations::CURRENT_SCHEMA_VERSION;
pub use repositories::{
    AuditEvent, AuditEventId, AuditEventRepository, JobId, JobMetadata, JobMetadataRepository,
    JobState, PlatformMetadataRepository, RuntimePolicyRecord, RuntimePolicyRepository,
    SettingsRepository,
};
pub use setup::{
    PersistedArtifactUpdate, PersistedDestinationCategory, PersistedEffectDisposition,
    PersistedModelArtifact, PersistedModelIntegrity, PersistedModelLifecycle,
    PersistedModelVerification, PersistedSetupApproval, PersistedSetupApprovalDecision,
    PersistedSetupApprovalInput, PersistedSetupEffect, PersistedSetupEffectInput,
    PersistedSetupEvent, PersistedSetupEventKind, PersistedSetupJob, PersistedSetupNotice,
    PersistedSetupPlan, PersistedSetupProgress, PersistedSetupStage, PersistedSetupState,
    PersistedSetupTransition, PersistedSetupWriteResult, SetupJobRepository,
};
