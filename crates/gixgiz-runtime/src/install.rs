//! Provider-neutral managed runtime installation.
//!
//! Core policy decides *whether* an approved installation may run. An adapter
//! decides *how* one provider is installed. Nothing here names an installer
//! executable, argument, registry key, or download URL.
//!
//! The trait is deliberately shaped so that verification cannot be skipped: an
//! adapter reports what it *observed* after installing, and core decides what
//! that means. An installer exit code is never sufficient evidence.

use gixgiz_contracts::{
    RuntimeInstallDestinationCategory, RuntimeInstallIntegrityEvidence, RuntimeInstallProgress,
    RuntimeInstallVerificationResult,
};
use tokio::sync::mpsc;

use crate::{RuntimeFuture, RuntimeOperationContext};

/// Recommended bounded capacity for normalized installer transfer progress.
///
/// Providers may coalesce or drop intermediate updates when this channel is
/// full. Durable job state remains authoritative.
pub const INSTALL_PROGRESS_CHANNEL_CAPACITY: usize = 16;

/// Bounded sender for provider-neutral installer transfer progress.
pub type InstallProgressSender = mpsc::Sender<RuntimeInstallProgress>;

/// Trusted, adapter-declared description of one installable runtime version.
///
/// An adapter returns this from a compiled allowlist. Core never accepts a
/// caller-supplied origin, artifact name, digest, or publisher.
// Deliberately exhaustive: adapters in other crates construct this value, so
// sealing it would make the trait unimplementable outside this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeInstallCandidate {
    /// Exact version this candidate installs.
    pub version: String,
    /// Bounded display origin shown to the user, without query parameters.
    pub source_origin: String,
    /// Bounded artifact file name.
    pub artifact_name: String,
    /// Conservative expected transfer size.
    pub expected_size_bytes: u64,
    /// Integrity evidence the adapter will enforce before any execution.
    pub integrity_evidence: RuntimeInstallIntegrityEvidence,
    /// Expected signing publisher shown to the user and enforced on the artifact.
    pub expected_publisher: String,
    /// Where the installation writes.
    pub destination: RuntimeInstallDestinationCategory,
    /// Whether the operating system will require administrator approval.
    ///
    /// A per-user installation is `false`. This never implies GixGiz approval.
    pub requires_administrator: bool,
}

/// Why an adapter refused to produce or accept an installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeInstallRefusal {
    /// No trusted allowlisted artifact exists for this platform.
    NoTrustedArtifact,
    /// Trustworthy integrity evidence could not be established.
    NoIntegrityEvidence,
    /// A runtime is already installed and is not owned by GixGiz.
    AlreadyInstalled,
    /// The provider cannot be installed by GixGiz on this platform.
    Unsupported,
}

/// Outcome of transferring and verifying an installer artifact.
///
/// A `Rejected` artifact must never be executed. Adapters are required to
/// verify *before* execution, not after.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeArtifactOutcome {
    /// The artifact was staged and every available integrity check passed.
    Verified {
        /// Bytes actually transferred into GixGiz-owned staging.
        transferred_bytes: u64,
    },
    /// The artifact failed verification and was quarantined, never executed.
    Rejected {
        /// Which check failed.
        reason: RuntimeArtifactRejection,
    },
}

/// Which artifact check failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeArtifactRejection {
    /// The origin was not the allowlisted trusted source.
    SourceUntrusted,
    /// The artifact exceeded its declared or absolute size bound.
    TooLarge,
    /// The digest did not match trusted expected evidence.
    DigestMismatch,
    /// No valid signature was present.
    SignatureInvalid,
    /// The signing publisher did not match the expectation.
    PublisherUnexpected,
    /// GixGiz-owned staging cannot hold the artifact.
    InsufficientStagingSpace,
    /// The installation destination cannot hold the installed runtime.
    InsufficientInstallSpace,
}

/// Result of running an approved provider installer.
///
/// Deliberately carries no notion of success beyond the provider's own report.
/// Core must still verify independently before recording any ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeInstallExecution {
    /// The installer reported that it completed.
    ReportedComplete,
    /// The installer reported an unsuccessful result.
    ReportedFailure,
    /// The installer outcome could not be established.
    Uncertain,
}

/// Installs a runtime after core has recorded an exact approval.
///
/// Implementations must not download, execute, or mutate anything before
/// `stage_artifact` is called, and must not execute an artifact whose
/// verification did not pass.
pub trait RuntimeInstaller: Send + Sync {
    /// Returns the trusted installable candidate for this platform.
    ///
    /// Performs no download, execution, or system change.
    fn candidate(
        &self,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, Result<RuntimeInstallCandidate, RuntimeInstallRefusal>>;

    /// Transfers the artifact into GixGiz-owned staging and verifies it.
    ///
    /// Must complete every available integrity check before returning
    /// `Verified`, and must never execute the artifact.
    fn stage_artifact(
        &self,
        candidate: &RuntimeInstallCandidate,
        progress: InstallProgressSender,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeArtifactOutcome>;

    /// Executes the verified staged installer.
    ///
    /// Must refuse unless `stage_artifact` previously returned `Verified` for
    /// this exact candidate.
    fn run_installer(
        &self,
        candidate: &RuntimeInstallCandidate,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeInstallExecution>;

    /// Independently verifies the installed runtime.
    ///
    /// Every field is set only from observed evidence. An installer exit code
    /// must never influence this result.
    fn verify_installation(
        &self,
        candidate: &RuntimeInstallCandidate,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeInstallVerificationResult>;

    /// Removes GixGiz-owned staging for this candidate.
    ///
    /// Must only ever remove artifacts GixGiz itself staged, and must never
    /// touch provider-owned installation or model data.
    fn discard_staged_artifact(
        &self,
        candidate: &RuntimeInstallCandidate,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, ()>;
}
