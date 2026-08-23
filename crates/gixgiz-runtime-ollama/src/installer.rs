//! Managed per-user Ollama installation for Windows.
//!
//! # Why there is no privileged helper here
//!
//! The Ollama Windows installer is an Inno Setup package that installs
//! **per user** into `%LOCALAPPDATA%\Programs\Ollama` and registers only under
//! `HKCU`. It requires no administrator rights. ADR 0007 §1 states that an
//! operation which can succeed safely per-user must not be elevated for
//! convenience, so this adapter deliberately never elevates and never launches
//! a privileged helper. `requires_administrator` is therefore `false`, and the
//! GixGiz plan approval remains the only approval that authorizes anything.
//!
//! # Integrity before execution
//!
//! The artifact is transferred into GixGiz-owned staging and then checked
//! against a pinned size bound, a pinned SHA-256 digest, and an Authenticode
//! signature whose publisher must be the trusted Ollama publisher. Only after
//! all of those pass may the installer be executed. Because integrity rests on
//! the digest and the signature rather than on the transport alone, bounded
//! redirects during transfer cannot substitute a different artifact.
//!
//! No `unsafe` and no additional dependency is used: transfer and hashing run
//! through the same fixed, non-interactive system PowerShell already audited
//! for Authenticode verification, with every caller-supplied value passed as a
//! hex token rather than interpolated into a script.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use gixgiz_contracts::{
    RuntimeInstallDestinationCategory, RuntimeInstallIntegrityEvidence,
    RuntimeInstallVerificationResult, RuntimeState,
};
use gixgiz_runtime::{
    InstallProgressSender, RuntimeArtifactOutcome, RuntimeArtifactRejection, RuntimeError,
    RuntimeFuture, RuntimeInstallCandidate, RuntimeInstallExecution, RuntimeInstallRefusal,
    RuntimeInstaller, RuntimeOperationContext,
};
use tokio::process::Command;

use crate::{
    discovery::ExecutableLocator,
    error::OllamaAdapterError,
    process::{ProcessLimits, verify_authenticode_path},
};

/// Pinned trusted installer artifact.
///
/// This is a compiled allowlist. No caller may supply an origin, artifact name,
/// version, or digest, and no "latest" URL is ever resolved at runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InstallerManifest {
    pub(crate) version: &'static str,
    pub(crate) origin: &'static str,
    pub(crate) url: &'static str,
    pub(crate) artifact_name: &'static str,
    pub(crate) expected_size_bytes: u64,
    pub(crate) maximum_size_bytes: u64,
    pub(crate) sha256: &'static str,
    pub(crate) publisher: &'static str,
    /// Conservative free space the installation destination needs.
    ///
    /// The artifact is compressed; the installed runtime ships many per-CPU and
    /// accelerator libraries and expands well beyond the download size.
    pub(crate) required_install_bytes: u64,
}

/// Required prefix for every trusted installer URL.
const TRUSTED_URL_PREFIX: &str = "https://github.com/ollama/ollama/releases/download/";

/// The v0.1 pinned Windows installer.
///
/// `sha256` must be the digest published in the matching release's
/// `sha256sum.txt`. An empty digest disables installation entirely rather than
/// weakening the integrity gate.
pub(crate) const V0_1_INSTALLER: InstallerManifest = InstallerManifest {
    // 0.32.5 is the top of the version range this release has evidence for, so a
    // freshly installed runtime is `Compatible` rather than merely untested.
    version: "0.32.5",
    origin: "https://github.com/ollama/ollama/releases",
    url: "https://github.com/ollama/ollama/releases/download/v0.32.5/OllamaSetup.exe",
    artifact_name: "OllamaSetup.exe",
    expected_size_bytes: 1_563_078_600,
    maximum_size_bytes: 4 * 1024 * 1024 * 1024,
    // Digest published in the v0.32.5 release `sha256sum.txt`.
    //
    // A wrong value here fails closed: the artifact is rejected and the
    // installer is never executed. Re-confirm this against the published file
    // whenever the pinned version changes.
    sha256: "b7eeef038ddcbd09ac665b11872baff1bc9b42794be41b5ef187b2f4b16a4498",
    publisher: "Ollama Inc.",
    required_install_bytes: 6 * 1024 * 1024 * 1024,
};

/// Inno Setup arguments for an unattended per-user installation.
///
/// Passed as separate arguments to a structured process API. No shell is
/// involved and no value is interpolated from caller input.
const INSTALLER_ARGUMENTS: [&str; 5] = [
    "/VERYSILENT",
    "/SUPPRESSMSGBOXES",
    "/NORESTART",
    "/SP-",
    "/NOCANCEL",
];

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const VERIFY_TIMEOUT: Duration = Duration::from_secs(60);
/// How often the staged artifact is measured while a transfer runs.
const PROGRESS_POLL_INTERVAL: Duration = Duration::from_millis(750);
const COMMAND_OUTPUT_LIMIT: usize = 8 * 1024;

/// Per-user Ollama installer.
pub struct OllamaInstaller {
    staging_root: PathBuf,
    /// User-selected installation directory, when one was chosen.
    install_root: Option<PathBuf>,
    manifest: InstallerManifest,
    locator: std::sync::Arc<dyn ExecutableLocator>,
    detector: std::sync::Arc<dyn gixgiz_runtime::RuntimeDetector>,
}

impl OllamaInstaller {
    /// Composes the v0.1 per-user installer for the signed-in user.
    ///
    /// `staging_root` must be the GixGiz-owned staging directory. `detector`
    /// supplies ordinary runtime detection so post-install verification reuses
    /// the same evidence the rest of the platform relies on.
    #[must_use]
    pub fn for_current_user(
        staging_root: PathBuf,
        install_root: Option<PathBuf>,
        detector: std::sync::Arc<dyn gixgiz_runtime::RuntimeDetector>,
    ) -> Self {
        Self {
            staging_root,
            install_root,
            manifest: V0_1_INSTALLER,
            locator: std::sync::Arc::new(crate::discovery::WindowsExecutableLocator::new(None)),
            detector,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_locator(
        staging_root: PathBuf,
        locator: std::sync::Arc<dyn ExecutableLocator>,
        detector: std::sync::Arc<dyn gixgiz_runtime::RuntimeDetector>,
    ) -> Self {
        Self {
            staging_root,
            install_root: None,
            manifest: V0_1_INSTALLER,
            locator,
            detector,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_manifest(mut self, manifest: InstallerManifest) -> Self {
        self.manifest = manifest;
        self
    }

    /// Returns the validated staged artifact path.
    ///
    /// The path is always a direct child of the canonical staging root, so a
    /// traversal or junction escape cannot place the artifact elsewhere.
    fn staged_path(&self) -> Result<PathBuf, OllamaAdapterError> {
        staged_artifact_path(&self.staging_root, self.manifest.artifact_name)
    }
}

/// Validates and builds the staged artifact path.
pub(crate) fn staged_artifact_path(
    staging_root: &Path,
    artifact_name: &str,
) -> Result<PathBuf, OllamaAdapterError> {
    if !staging_root.is_absolute() {
        return Err(OllamaAdapterError::InvalidExecutable);
    }
    // The artifact name comes from the compiled manifest, but validate anyway so
    // a future manifest edit cannot introduce traversal.
    if artifact_name.is_empty()
        || artifact_name.len() > 128
        || artifact_name.contains(['/', '\\', ':', '\0'])
        || artifact_name.contains("..")
    {
        return Err(OllamaAdapterError::InvalidExecutable);
    }
    let canonical =
        std::fs::canonicalize(staging_root).map_err(|_| OllamaAdapterError::InvalidExecutable)?;
    let candidate = canonical.join(artifact_name);
    if candidate.parent() != Some(canonical.as_path()) {
        return Err(OllamaAdapterError::InvalidExecutable);
    }
    Ok(candidate)
}

/// Returns the per-user application root the provider installer writes into.
fn install_destination_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(|value| PathBuf::from(value).join("Programs"))
}

/// Validates that a manifest can be trusted before any transfer occurs.
pub(crate) fn manifest_is_trustworthy(manifest: &InstallerManifest) -> bool {
    manifest.url.starts_with(TRUSTED_URL_PREFIX)
        && !manifest.sha256.is_empty()
        && manifest.sha256.len() == 64
        && manifest.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        && manifest.maximum_size_bytes > 0
        && manifest.expected_size_bytes <= manifest.maximum_size_bytes
        && !manifest.publisher.is_empty()
}

impl RuntimeInstaller for OllamaInstaller {
    fn candidate(
        &self,
        _context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, Result<RuntimeInstallCandidate, RuntimeInstallRefusal>> {
        Box::pin(async move {
            // Refuse before doing anything when the pinned artifact carries no
            // trustworthy integrity evidence.
            if !manifest_is_trustworthy(&self.manifest) {
                return Ok(Err(RuntimeInstallRefusal::NoIntegrityEvidence));
            }
            if !cfg!(windows) {
                return Ok(Err(RuntimeInstallRefusal::Unsupported));
            }
            // An existing installation is reused, never replaced.
            if matches!(self.locator.locate(), Ok(Some(_))) {
                return Ok(Err(RuntimeInstallRefusal::AlreadyInstalled));
            }
            Ok(Ok(RuntimeInstallCandidate {
                version: self.manifest.version.to_owned(),
                source_origin: self.manifest.origin.to_owned(),
                artifact_name: self.manifest.artifact_name.to_owned(),
                expected_size_bytes: self.manifest.expected_size_bytes,
                integrity_evidence: RuntimeInstallIntegrityEvidence::DigestAndPublisher,
                expected_publisher: self.manifest.publisher.to_owned(),
                destination: RuntimeInstallDestinationCategory::PerUserApplicationDirectory,
                // Per-user Inno Setup installation: no elevation is requested.
                requires_administrator: false,
            }))
        })
    }

    fn stage_artifact(
        &self,
        _candidate: &RuntimeInstallCandidate,
        progress: InstallProgressSender,
        _context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeArtifactOutcome> {
        Box::pin(async move {
            if !manifest_is_trustworthy(&self.manifest) {
                return Ok(RuntimeArtifactOutcome::Rejected {
                    reason: RuntimeArtifactRejection::SourceUntrusted,
                });
            }
            let destination = self.staged_path().map_err(|_| RuntimeError::InvalidInput)?;

            // Check both drives before transferring a large artifact. Staging and
            // the installation destination can be different volumes, and running
            // out of room mid-install wastes the whole download.
            let space_limits = ProcessLimits {
                timeout: VERIFY_TIMEOUT,
                max_output_bytes: COMMAND_OUTPUT_LIMIT,
            };
            if let Ok(staging_free) =
                crate::transfer::free_space_bytes(&self.staging_root, space_limits).await
                && staging_free < self.manifest.expected_size_bytes
            {
                return Ok(RuntimeArtifactOutcome::Rejected {
                    reason: RuntimeArtifactRejection::InsufficientStagingSpace,
                });
            }
            if let Some(install_root) = self.install_root.clone().or_else(install_destination_root)
                && let Ok(install_free) =
                    crate::transfer::free_space_bytes(&install_root, space_limits).await
                && install_free < self.manifest.required_install_bytes
            {
                return Ok(RuntimeArtifactOutcome::Rejected {
                    reason: RuntimeArtifactRejection::InsufficientInstallSpace,
                });
            }

            // The transfer runs in a child process that reports nothing until it
            // exits, so observe the staged file directly. This makes a large
            // download visible without parsing untrusted process output.
            let watched = destination.clone();
            let expected = self.manifest.expected_size_bytes;
            let reporter = progress.clone();
            let watcher = tokio::spawn(async move {
                let mut last = 0_u64;
                loop {
                    tokio::time::sleep(PROGRESS_POLL_INTERVAL).await;
                    let Ok(metadata) = std::fs::metadata(&watched) else {
                        continue;
                    };
                    let transferred = metadata.len();
                    if transferred == last {
                        continue;
                    }
                    last = transferred;
                    let update = gixgiz_contracts::RuntimeInstallProgress {
                        transferred_bytes: transferred,
                        expected_bytes: Some(expected),
                    };
                    if reporter.send(update).await.is_err() {
                        return;
                    }
                }
            });

            let transfer = crate::transfer::download_and_hash(
                self.manifest.url,
                &destination,
                self.manifest.maximum_size_bytes,
                ProcessLimits {
                    timeout: TRANSFER_TIMEOUT,
                    max_output_bytes: COMMAND_OUTPUT_LIMIT,
                },
            )
            .await;
            watcher.abort();

            let transfer = match transfer {
                Ok(transfer) => transfer,
                Err(_) => {
                    let _ = std::fs::remove_file(&destination);
                    return Err(RuntimeError::ProviderUnavailable);
                }
            };

            if transfer.bytes > self.manifest.maximum_size_bytes {
                let _ = std::fs::remove_file(&destination);
                return Ok(RuntimeArtifactOutcome::Rejected {
                    reason: RuntimeArtifactRejection::TooLarge,
                });
            }
            if !transfer.sha256.eq_ignore_ascii_case(self.manifest.sha256) {
                let _ = std::fs::remove_file(&destination);
                return Ok(RuntimeArtifactOutcome::Rejected {
                    reason: RuntimeArtifactRejection::DigestMismatch,
                });
            }

            // Signature and publisher are checked before the file is ever run.
            match verify_authenticode_path(
                &destination,
                ProcessLimits {
                    timeout: VERIFY_TIMEOUT,
                    max_output_bytes: COMMAND_OUTPUT_LIMIT,
                },
            )
            .await
            {
                Ok(()) => {}
                Err(OllamaAdapterError::ExecutableUntrusted) => {
                    let _ = std::fs::remove_file(&destination);
                    return Ok(RuntimeArtifactOutcome::Rejected {
                        reason: RuntimeArtifactRejection::PublisherUnexpected,
                    });
                }
                Err(_) => {
                    let _ = std::fs::remove_file(&destination);
                    return Ok(RuntimeArtifactOutcome::Rejected {
                        reason: RuntimeArtifactRejection::SignatureInvalid,
                    });
                }
            }

            let update = gixgiz_contracts::RuntimeInstallProgress {
                transferred_bytes: transfer.bytes,
                expected_bytes: Some(self.manifest.expected_size_bytes),
            };
            let _ = progress.send(update).await;

            Ok(RuntimeArtifactOutcome::Verified {
                transferred_bytes: transfer.bytes,
            })
        })
    }

    fn run_installer(
        &self,
        _candidate: &RuntimeInstallCandidate,
        _context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeInstallExecution> {
        Box::pin(async move {
            let staged = self.staged_path().map_err(|_| RuntimeError::InvalidInput)?;

            // Re-verify immediately before execution. Verification and execution
            // are separate calls, so this closes the gap between them.
            let limits = ProcessLimits {
                timeout: VERIFY_TIMEOUT,
                max_output_bytes: COMMAND_OUTPUT_LIMIT,
            };
            if verify_authenticode_path(&staged, limits).await.is_err() {
                return Err(RuntimeError::InvalidInput);
            }

            let mut command = Command::new(&staged);
            for argument in INSTALLER_ARGUMENTS {
                command.arg(argument);
            }
            // A chosen directory is passed as one fixed switch with the path as
            // its value; nothing is interpolated and no shell is involved.
            if let Some(install_root) = self.install_root.as_ref() {
                let mut switch = std::ffi::OsString::from("/DIR=");
                switch.push(install_root.as_os_str());
                command.arg(switch);
            }
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(false);

            let Ok(mut child) = command.spawn() else {
                return Ok(RuntimeInstallExecution::ReportedFailure);
            };
            match tokio::time::timeout(INSTALL_TIMEOUT, child.wait()).await {
                Ok(Ok(status)) if status.success() => Ok(RuntimeInstallExecution::ReportedComplete),
                Ok(Ok(_)) => Ok(RuntimeInstallExecution::ReportedFailure),
                // A vanished or timed-out installer leaves genuinely unknown
                // system state; it is never guessed in either direction.
                Ok(Err(_)) | Err(_) => Ok(RuntimeInstallExecution::Uncertain),
            }
        })
    }

    fn verify_installation(
        &self,
        _candidate: &RuntimeInstallCandidate,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeInstallVerificationResult> {
        Box::pin(async move {
            let mut result = RuntimeInstallVerificationResult::default();

            let Ok(Some(executable)) = self.locator.locate() else {
                return Ok(result);
            };
            result.executable_located = true;

            // Signature and publisher of what was actually installed.
            let limits = ProcessLimits {
                timeout: VERIFY_TIMEOUT,
                max_output_bytes: COMMAND_OUTPUT_LIMIT,
            };
            if verify_authenticode_path(executable.path(), limits)
                .await
                .is_ok()
            {
                result.signature_valid = true;
                result.publisher_matched = true;
            }

            // Version and endpoint health come from ordinary detection, so this
            // reuses the same evidence the rest of the platform relies on.
            if let Ok(observation) = self.detector.detect(context).await {
                result.version_supported = matches!(
                    observation.version.as_ref().map(|info| info.compatibility),
                    Some(gixgiz_contracts::RuntimeVersionCompatibility::Compatible)
                );
                result.endpoint_healthy = matches!(
                    observation.state,
                    RuntimeState::Ready | RuntimeState::Degraded
                ) && observation.endpoint_safety
                    == gixgiz_contracts::RuntimeEndpointSafety::LoopbackVerified;
            }

            Ok(result)
        })
    }

    fn discard_staged_artifact(
        &self,
        _candidate: &RuntimeInstallCandidate,
        _context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, ()> {
        Box::pin(async move {
            // Only ever removes the exact artifact GixGiz staged. Provider
            // installation and model data are never touched.
            if let Ok(staged) = self.staged_path() {
                let _ = std::fs::remove_file(staged);
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Detector stub: `candidate` never consults detection, so this is unused
    /// evidence that still satisfies the composition.
    struct UndetectableRuntime;

    impl gixgiz_runtime::RuntimeDetector for UndetectableRuntime {
        fn detect(
            &self,
            _context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, gixgiz_runtime::RuntimeObservation> {
            Box::pin(async { Err(RuntimeError::ProviderUnavailable) })
        }
    }

    fn test_context() -> RuntimeOperationContext {
        RuntimeOperationContext::new(
            gixgiz_contracts::CorrelationId::new(),
            gixgiz_contracts::RequestId::new(),
            Duration::from_secs(5),
        )
    }

    fn manifest() -> InstallerManifest {
        InstallerManifest {
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ..V0_1_INSTALLER
        }
    }

    #[test]
    fn the_shipped_manifest_carries_complete_integrity_evidence() {
        // A pinned digest and publisher must both be present, or installation
        // is refused rather than proceeding without integrity evidence.
        assert_eq!(V0_1_INSTALLER.sha256.len(), 64);
        assert!(V0_1_INSTALLER.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(!V0_1_INSTALLER.publisher.is_empty());
        assert!(manifest_is_trustworthy(&V0_1_INSTALLER));
    }

    #[test]
    fn the_pinned_version_sits_inside_the_supported_range() {
        // Installing a version outside recorded evidence would land the user in
        // a degraded runtime immediately after a successful installation.
        let pinned = semver::Version::parse(V0_1_INSTALLER.version).expect("pinned version parses");
        let policy = crate::version::VersionPolicy::v0_1();
        let (_, support) = policy
            .assess(V0_1_INSTALLER.version)
            .expect("pinned version is assessable");
        assert_eq!(
            support,
            crate::version::VersionSupport::Compatible,
            "pinned {pinned} must be a supported version"
        );
    }

    #[test]
    fn a_complete_manifest_is_trusted() {
        assert!(manifest_is_trustworthy(&manifest()));
    }

    #[test]
    fn an_untrusted_origin_is_refused() {
        let hostile = InstallerManifest {
            url: "https://example.invalid/OllamaSetup.exe",
            ..manifest()
        };
        assert!(!manifest_is_trustworthy(&hostile));
    }

    #[test]
    fn a_malformed_digest_is_refused() {
        for digest in [
            "",
            "abc",
            "zzzz56789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0",
        ] {
            let broken = InstallerManifest {
                sha256: digest,
                ..manifest()
            };
            assert!(!manifest_is_trustworthy(&broken), "{digest:?}");
        }
    }

    #[test]
    fn the_pinned_url_lives_under_the_trusted_release_prefix() {
        assert!(V0_1_INSTALLER.url.starts_with(TRUSTED_URL_PREFIX));
        assert!(V0_1_INSTALLER.url.ends_with("/OllamaSetup.exe"));
    }

    #[test]
    fn installer_arguments_are_fixed_and_carry_no_interpolation() {
        for argument in INSTALLER_ARGUMENTS {
            assert!(argument.starts_with('/'));
            assert!(!argument.contains(' '));
            assert!(!argument.contains('&'));
            assert!(!argument.contains('%'));
            assert!(!argument.contains('$'));
        }
    }

    #[test]
    fn a_staged_artifact_cannot_escape_the_staging_root() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path();

        for hostile in [
            "..\\evil.exe",
            "../evil.exe",
            "sub\\evil.exe",
            "sub/evil.exe",
            "C:\\evil.exe",
            "",
        ] {
            assert!(
                staged_artifact_path(root, hostile).is_err(),
                "{hostile:?} must be rejected"
            );
        }

        let good = staged_artifact_path(root, "OllamaSetup.exe").expect("plain name is accepted");
        assert_eq!(
            good.file_name().and_then(|name| name.to_str()),
            Some("OllamaSetup.exe")
        );
        assert_eq!(
            good.parent(),
            Some(
                std::fs::canonicalize(root)
                    .expect("canonical root")
                    .as_path()
            )
        );
    }

    #[tokio::test]
    async fn an_untrusted_manifest_refuses_before_any_transfer() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let installer = OllamaInstaller::with_locator(
            temporary.path().to_path_buf(),
            crate::discovery::FixedExecutableLocator::new(Ok(None)),
            std::sync::Arc::new(UndetectableRuntime),
        )
        .with_manifest(InstallerManifest {
            sha256: "",
            ..V0_1_INSTALLER
        });

        // Without a pinned digest there is no integrity evidence, so no plan is
        // offered and nothing is transferred.
        let refusal = installer
            .candidate(test_context())
            .await
            .expect("candidate resolves")
            .expect_err("an untrusted manifest is refused");

        assert_eq!(refusal, RuntimeInstallRefusal::NoIntegrityEvidence);
    }

    #[tokio::test]
    async fn an_existing_installation_is_never_replaced() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let executable = temporary.path().join("ollama.exe");
        std::fs::write(&executable, b"stub").expect("stub executable is written");
        let installer = OllamaInstaller::with_locator(
            temporary.path().to_path_buf(),
            crate::discovery::FixedExecutableLocator::from_path(&executable),
            std::sync::Arc::new(UndetectableRuntime),
        )
        .with_manifest(manifest());

        let refusal = installer
            .candidate(test_context())
            .await
            .expect("candidate resolves")
            .expect_err("an existing runtime is reused, not replaced");

        assert_eq!(refusal, RuntimeInstallRefusal::AlreadyInstalled);
    }

    #[test]
    fn a_relative_staging_root_is_rejected() {
        assert!(staged_artifact_path(Path::new("relative"), "OllamaSetup.exe").is_err());
    }
}
