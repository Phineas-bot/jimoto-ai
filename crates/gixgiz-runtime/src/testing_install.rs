//! Deterministic fake runtime installer for tests and CI.
//!
//! This fake exists so the entire installation state machine can be exercised
//! without administrator rights, network access, a real installer, or any
//! change to Windows installation state.
//!
//! It deliberately records call order so tests can assert the security-critical
//! property that a rejected artifact is *never* executed.

use std::sync::{Arc, Mutex};

use gixgiz_contracts::{
    RuntimeInstallDestinationCategory, RuntimeInstallIntegrityEvidence, RuntimeInstallProgress,
    RuntimeInstallVerificationResult,
};

use crate::{
    InstallProgressSender, RuntimeArtifactOutcome, RuntimeFuture, RuntimeInstallCandidate,
    RuntimeInstallExecution, RuntimeInstallRefusal, RuntimeInstaller, RuntimeOperationContext,
};

/// One recorded interaction with the fake installer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FakeInstallCall {
    /// The trusted candidate was requested.
    Candidate,
    /// Artifact transfer and verification was requested.
    StageArtifact,
    /// Installer execution was requested.
    RunInstaller,
    /// Independent post-installation verification was requested.
    VerifyInstallation,
    /// Staged-artifact cleanup was requested.
    DiscardStagedArtifact,
}

/// Deterministic in-memory runtime installer.
pub struct FakeRuntimeInstaller {
    candidate: Arc<Mutex<Result<RuntimeInstallCandidate, RuntimeInstallRefusal>>>,
    artifact: Arc<Mutex<RuntimeArtifactOutcome>>,
    execution: Arc<Mutex<RuntimeInstallExecution>>,
    verification: Arc<Mutex<RuntimeInstallVerificationResult>>,
    progress: Arc<Mutex<Vec<RuntimeInstallProgress>>>,
    calls: Arc<Mutex<Vec<FakeInstallCall>>>,
}

impl FakeRuntimeInstaller {
    /// Creates a fake whose whole flow succeeds and verifies.
    #[must_use]
    pub fn succeeding() -> Self {
        Self {
            candidate: Arc::new(Mutex::new(Ok(sample_candidate()))),
            artifact: Arc::new(Mutex::new(RuntimeArtifactOutcome::Verified {
                transferred_bytes: 1024,
            })),
            execution: Arc::new(Mutex::new(RuntimeInstallExecution::ReportedComplete)),
            verification: Arc::new(Mutex::new(RuntimeInstallVerificationResult {
                executable_located: true,
                signature_valid: true,
                publisher_matched: true,
                version_supported: true,
                endpoint_healthy: true,
            })),
            progress: Arc::new(Mutex::new(Vec::new())),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Replaces the trusted candidate result.
    #[must_use]
    pub fn with_candidate(
        self,
        candidate: Result<RuntimeInstallCandidate, RuntimeInstallRefusal>,
    ) -> Self {
        *self.candidate.lock().expect("candidate lock") = candidate;
        self
    }

    /// Replaces the artifact verification outcome.
    #[must_use]
    pub fn with_artifact(self, artifact: RuntimeArtifactOutcome) -> Self {
        *self.artifact.lock().expect("artifact lock") = artifact;
        self
    }

    /// Replaces the installer execution result.
    #[must_use]
    pub fn with_execution(self, execution: RuntimeInstallExecution) -> Self {
        *self.execution.lock().expect("execution lock") = execution;
        self
    }

    /// Replaces the independent verification evidence.
    #[must_use]
    pub fn with_verification(self, verification: RuntimeInstallVerificationResult) -> Self {
        *self.verification.lock().expect("verification lock") = verification;
        self
    }

    /// Returns recorded calls in order.
    #[must_use]
    pub fn calls(&self) -> Vec<FakeInstallCall> {
        self.calls.lock().expect("calls lock").clone()
    }

    /// Returns whether the installer was ever executed.
    #[must_use]
    pub fn installer_was_executed(&self) -> bool {
        self.calls().contains(&FakeInstallCall::RunInstaller)
    }

    /// Returns progress updates published during staging.
    #[must_use]
    pub fn progress(&self) -> Vec<RuntimeInstallProgress> {
        self.progress.lock().expect("progress lock").clone()
    }

    fn record(&self, call: FakeInstallCall) {
        self.calls.lock().expect("calls lock").push(call);
    }
}

impl Default for FakeRuntimeInstaller {
    fn default() -> Self {
        Self::succeeding()
    }
}

/// Returns a per-user candidate that requires no administrator rights.
#[must_use]
pub fn sample_candidate() -> RuntimeInstallCandidate {
    RuntimeInstallCandidate {
        version: "1.2.3".to_owned(),
        source_origin: "https://example.invalid".to_owned(),
        artifact_name: "TestRuntimeSetup.exe".to_owned(),
        expected_size_bytes: 1024,
        integrity_evidence: RuntimeInstallIntegrityEvidence::DigestAndPublisher,
        expected_publisher: "Test Publisher".to_owned(),
        destination: RuntimeInstallDestinationCategory::PerUserApplicationDirectory,
        requires_administrator: false,
    }
}

impl RuntimeInstaller for FakeRuntimeInstaller {
    fn candidate(
        &self,
        _context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, Result<RuntimeInstallCandidate, RuntimeInstallRefusal>> {
        self.record(FakeInstallCall::Candidate);
        let candidate = self.candidate.lock().expect("candidate lock").clone();
        Box::pin(async move { Ok(candidate) })
    }

    fn stage_artifact(
        &self,
        candidate: &RuntimeInstallCandidate,
        progress: InstallProgressSender,
        _context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeArtifactOutcome> {
        self.record(FakeInstallCall::StageArtifact);
        let outcome = self.artifact.lock().expect("artifact lock").clone();
        let expected = candidate.expected_size_bytes;
        let recorded = Arc::clone(&self.progress);
        Box::pin(async move {
            if let RuntimeArtifactOutcome::Verified { transferred_bytes } = outcome {
                let update = RuntimeInstallProgress {
                    transferred_bytes,
                    expected_bytes: Some(expected),
                };
                recorded.lock().expect("progress lock").push(update);
                let _ = progress.send(update).await;
            }
            Ok(outcome)
        })
    }

    fn run_installer(
        &self,
        _candidate: &RuntimeInstallCandidate,
        _context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeInstallExecution> {
        self.record(FakeInstallCall::RunInstaller);
        let execution = *self.execution.lock().expect("execution lock");
        Box::pin(async move { Ok(execution) })
    }

    fn verify_installation(
        &self,
        _candidate: &RuntimeInstallCandidate,
        _context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeInstallVerificationResult> {
        self.record(FakeInstallCall::VerifyInstallation);
        let verification = *self.verification.lock().expect("verification lock");
        Box::pin(async move { Ok(verification) })
    }

    fn discard_staged_artifact(
        &self,
        _candidate: &RuntimeInstallCandidate,
        _context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, ()> {
        self.record(FakeInstallCall::DiscardStagedArtifact);
        Box::pin(async move { Ok(()) })
    }
}
