use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
    sync::Mutex,
    time,
};

use crate::{
    discovery::ValidatedExecutable,
    endpoint::{OLLAMA_HOST_ENV, ValidatedEndpoint},
    error::OllamaAdapterError,
    version::parse_version,
};

const VERSION_ARGUMENT: &str = "--version";
const SERVE_ARGUMENT: &str = "serve";
const CLIENT_VERSION_PREFIX: &str = "Warning: client version is ";
const SERVER_VERSION_PREFIX: &str = "ollama version is ";
const AUTHENTICODE_TIMEOUT: Duration = Duration::from_secs(3);
const AUTHENTICODE_OUTPUT_LIMIT: usize = 256;
const AUTHENTICODE_PATH_WIDE_LIMIT: usize = 4_096;
const AUTHENTICODE_PATH_TOKEN_LIMIT: usize = 1 + (AUTHENTICODE_PATH_WIDE_LIMIT * 4);
const TRUSTED_PUBLISHER: &str = "Ollama Inc.";
const TRUSTED_PUBLISHER_KEY: &str = "ollamainc";
#[cfg(windows)]
const SYSTEM_POWERSHELL_PATH: &str =
    r"\\?\GLOBALROOT\SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe";
const POWERSHELL_SUFFIX: &str = r"System32\WindowsPowerShell\v1.0\powershell.exe";
const AUTHENTICODE_SCRIPT_TEMPLATE: &str = r#"& {
    param(
        [Parameter(Mandatory = $true, Position = 0)]
        [ValidatePattern('^x[0-9A-F]+$')]
        [string] $TargetPathToken
    )
    $ErrorActionPreference = 'Stop'
    if ($TargetPathToken.Length -gt __MAX_PATH_TOKEN_LENGTH__) {
        throw 'The target path token exceeds the fixed limit.'
    }
    $targetPathHex = $TargetPathToken.Substring(1)
    if (($targetPathHex.Length % 4) -ne 0) {
        throw 'The target path token is malformed.'
    }
    $targetPathChars = for ($index = 0; $index -lt $targetPathHex.Length; $index += 4) {
        [char][Convert]::ToUInt16($targetPathHex.Substring($index, 4), 16)
    }
    $TargetPath = -join $targetPathChars
    $securityModule = [System.IO.Path]::Combine(
        $PSHOME,
        'Modules',
        'Microsoft.PowerShell.Security',
        'Microsoft.PowerShell.Security.psd1'
    )
    Microsoft.PowerShell.Core\Import-Module -Name $securityModule -Force
    $signature = Microsoft.PowerShell.Security\Get-AuthenticodeSignature -LiteralPath $TargetPath
    $signatureValid = $signature.Status -eq [System.Management.Automation.SignatureStatus]::Valid
    $publisherValid = $false
    if ($null -ne $signature.SignerCertificate) {
        $publisher = $signature.SignerCertificate.GetNameInfo(
            [System.Security.Cryptography.X509Certificates.X509NameType]::SimpleName,
            $false
        )
        $publisherNormalized = ($publisher -replace '[\x09-\x0D\x20-\x2F\x3A-\x40\x5B-\x60\x7B-\x7E]', '').ToLowerInvariant()
        $publisherValid = $publisherNormalized -ceq '__TRUSTED_PUBLISHER_KEY__'
    }
    if ($signatureValid -and $publisherValid) {
        [Console]::Out.Write('{"signature_valid":true,"publisher_valid":true}')
    } elseif ($signatureValid) {
        [Console]::Out.Write('{"signature_valid":true,"publisher_valid":false}')
    } elseif ($publisherValid) {
        [Console]::Out.Write('{"signature_valid":false,"publisher_valid":true}')
    } else {
        [Console]::Out.Write('{"signature_valid":false,"publisher_valid":false}')
    }
}"#;

pub(crate) type ProcessFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, OllamaAdapterError>> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessLimits {
    pub(crate) timeout: Duration,
    pub(crate) max_output_bytes: usize,
}

pub(crate) trait OwnedProcessControl: Send + Sync {
    fn client_version<'a>(
        &'a self,
        executable: &'a ValidatedExecutable,
        endpoint: &'a ValidatedEndpoint,
        limits: ProcessLimits,
    ) -> ProcessFuture<'a, semver::Version>;

    fn start_owned<'a>(
        &'a self,
        executable: &'a ValidatedExecutable,
        endpoint: &'a ValidatedEndpoint,
    ) -> ProcessFuture<'a, u32>;

    fn stop_owned<'a>(&'a self, timeout: Duration) -> ProcessFuture<'a, ()>;

    fn owned_status<'a>(&'a self) -> ProcessFuture<'a, OwnedProcessStatus>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnedProcessStatus {
    None,
    Running,
    Exited,
}

trait ExecutableIdentityVerifier: Send + Sync {
    fn verify<'a>(
        &'a self,
        executable: &'a ValidatedExecutable,
        limits: ProcessLimits,
    ) -> ProcessFuture<'a, ()>;
}

#[derive(Debug, Default)]
struct PowerShellAuthenticodeVerifier;

impl ExecutableIdentityVerifier for PowerShellAuthenticodeVerifier {
    fn verify<'a>(
        &'a self,
        executable: &'a ValidatedExecutable,
        limits: ProcessLimits,
    ) -> ProcessFuture<'a, ()> {
        Box::pin(verify_authenticode(executable, limits))
    }
}

/// Tokio process supervisor retaining authority over only the exact child it spawned.
pub(crate) struct TokioOwnedProcessControl {
    child: Mutex<Option<Child>>,
    identity_verifier: Arc<dyn ExecutableIdentityVerifier>,
}

impl Default for TokioOwnedProcessControl {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
            identity_verifier: Arc::new(PowerShellAuthenticodeVerifier),
        }
    }
}

impl TokioOwnedProcessControl {
    pub(crate) fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    #[cfg(test)]
    fn with_identity_verifier(identity_verifier: Arc<dyn ExecutableIdentityVerifier>) -> Self {
        Self {
            child: Mutex::new(None),
            identity_verifier,
        }
    }
}

impl OwnedProcessControl for TokioOwnedProcessControl {
    fn client_version<'a>(
        &'a self,
        executable: &'a ValidatedExecutable,
        endpoint: &'a ValidatedEndpoint,
        limits: ProcessLimits,
    ) -> ProcessFuture<'a, semver::Version> {
        Box::pin(async move {
            let started = Instant::now();
            self.identity_verifier
                .verify(
                    executable,
                    ProcessLimits {
                        timeout: AUTHENTICODE_TIMEOUT.min(limits.timeout),
                        max_output_bytes: AUTHENTICODE_OUTPUT_LIMIT,
                    },
                )
                .await?;
            let remaining = limits.timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(OllamaAdapterError::TimedOut);
            }
            probe_client_version(
                executable,
                endpoint,
                ProcessLimits {
                    timeout: remaining,
                    max_output_bytes: limits.max_output_bytes,
                },
            )
            .await
        })
    }

    fn start_owned<'a>(
        &'a self,
        executable: &'a ValidatedExecutable,
        endpoint: &'a ValidatedEndpoint,
    ) -> ProcessFuture<'a, u32> {
        Box::pin(async move {
            let mut state = self.child.lock().await;
            if let Some(child) = state.as_mut() {
                match child.try_wait() {
                    Ok(None) => return Err(OllamaAdapterError::LifecycleConflict),
                    Ok(Some(_)) => *state = None,
                    Err(_) => return Err(OllamaAdapterError::ProcessControlFailed),
                }
            }

            self.identity_verifier
                .verify(
                    executable,
                    ProcessLimits {
                        timeout: AUTHENTICODE_TIMEOUT,
                        max_output_bytes: AUTHENTICODE_OUTPUT_LIMIT,
                    },
                )
                .await?;

            let mut command = Command::new(executable.path());
            command
                .arg(SERVE_ARGUMENT)
                .env(OLLAMA_HOST_ENV, endpoint.child_environment_value())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            let child = command
                .spawn()
                .map_err(|_| OllamaAdapterError::ProcessStartFailed)?;
            let process_id = child.id().ok_or(OllamaAdapterError::ProcessStartFailed)?;
            *state = Some(child);
            Ok(process_id)
        })
    }

    fn stop_owned<'a>(&'a self, timeout: Duration) -> ProcessFuture<'a, ()> {
        Box::pin(async move {
            let mut state = self.child.lock().await;
            let child = state
                .as_mut()
                .ok_or(OllamaAdapterError::LifecycleUnsupported)?;
            let result = time::timeout(timeout, async {
                child
                    .kill()
                    .await
                    .map_err(|_| OllamaAdapterError::ProcessControlFailed)?;
                child
                    .wait()
                    .await
                    .map_err(|_| OllamaAdapterError::ProcessControlFailed)?;
                Ok(())
            })
            .await
            .map_err(|_| OllamaAdapterError::TimedOut)?;
            *state = None;
            result
        })
    }

    fn owned_status<'a>(&'a self) -> ProcessFuture<'a, OwnedProcessStatus> {
        Box::pin(async move {
            let mut state = self.child.lock().await;
            let Some(child) = state.as_mut() else {
                return Ok(OwnedProcessStatus::None);
            };
            match child.try_wait() {
                Ok(None) => Ok(OwnedProcessStatus::Running),
                Ok(Some(_)) => {
                    *state = None;
                    Ok(OwnedProcessStatus::Exited)
                }
                Err(_) => Err(OllamaAdapterError::ProcessControlFailed),
            }
        })
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthenticodeDecision {
    signature_valid: bool,
    publisher_valid: bool,
}

async fn verify_authenticode(
    executable: &ValidatedExecutable,
    limits: ProcessLimits,
) -> Result<(), OllamaAdapterError> {
    let powershell = resolve_system_powershell()?;
    let mut command = authenticode_command(&powershell, executable.path())?;
    let mut child = command
        .spawn()
        .map_err(|error| OllamaAdapterError::executable_io("verify Authenticode", &error))?;
    let stdout = child.stdout.take().ok_or(OllamaAdapterError::Internal)?;
    let stderr = child.stderr.take().ok_or(OllamaAdapterError::Internal)?;
    let collected = collect_bounded_output(
        stdout,
        stderr,
        async {
            child.wait().await.map_err(|error| {
                OllamaAdapterError::executable_io("wait for Authenticode verification", &error)
            })
        },
        limits,
    )
    .await;
    let (stdout, stderr, status) = match collected {
        Ok(collected) => collected,
        Err(error) => {
            let _ = child.start_kill();
            return Err(error);
        }
    };

    validate_authenticode_output(&stdout, &stderr, status.success())
}

fn authenticode_command(
    powershell: &Path,
    executable: &Path,
) -> Result<Command, OllamaAdapterError> {
    let powershell_home = powershell
        .parent()
        .ok_or(OllamaAdapterError::InvalidExecutable)?;
    let normalized = normalize_publisher_name(TRUSTED_PUBLISHER);
    let publisher_key = if normalized.as_deref() == Some(TRUSTED_PUBLISHER_KEY) {
        TRUSTED_PUBLISHER_KEY
    } else {
        "__invalid_publisher_policy__"
    };
    let script = AUTHENTICODE_SCRIPT_TEMPLATE
        .replace("__TRUSTED_PUBLISHER_KEY__", publisher_key)
        .replace(
            "__MAX_PATH_TOKEN_LENGTH__",
            &AUTHENTICODE_PATH_TOKEN_LIMIT.to_string(),
        );
    let executable_token = encode_executable_path(executable)?;
    let mut command = Command::new(powershell);
    command
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(script)
        .arg(executable_token)
        .current_dir(powershell_home)
        .env("PSModulePath", powershell_home.join("Modules"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    Ok(command)
}

fn encode_executable_path(path: &Path) -> Result<String, OllamaAdapterError> {
    #[cfg(windows)]
    let wide = path
        .as_os_str()
        .encode_wide()
        .take(AUTHENTICODE_PATH_WIDE_LIMIT + 1)
        .collect::<Vec<_>>();
    #[cfg(not(windows))]
    let wide = path
        .as_os_str()
        .to_str()
        .ok_or(OllamaAdapterError::InvalidExecutable)?
        .encode_utf16()
        .take(AUTHENTICODE_PATH_WIDE_LIMIT + 1)
        .collect::<Vec<_>>();

    if wide.is_empty() || wide.len() > AUTHENTICODE_PATH_WIDE_LIMIT || wide.contains(&0) {
        return Err(OllamaAdapterError::InvalidExecutable);
    }
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut token = String::with_capacity(1 + (wide.len() * 4));
    token.push('x');
    for unit in wide {
        for shift in [12, 8, 4, 0] {
            token.push(char::from(HEX[((unit >> shift) & 0x0f) as usize]));
        }
    }
    Ok(token)
}

fn normalize_publisher_name(value: &str) -> Option<String> {
    let mut normalized = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else if !(character.is_ascii_punctuation() || character.is_ascii_whitespace()) {
            return None;
        }
    }
    (!normalized.is_empty()).then_some(normalized)
}

#[cfg(windows)]
fn resolve_system_powershell() -> Result<PathBuf, OllamaAdapterError> {
    let candidate = PathBuf::from(SYSTEM_POWERSHELL_PATH);
    let candidate_metadata =
        std::fs::symlink_metadata(&candidate).map_err(|_| OllamaAdapterError::InvalidExecutable)?;
    if !candidate_metadata.is_file() || candidate_metadata.file_type().is_symlink() {
        return Err(OllamaAdapterError::InvalidExecutable);
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|_| OllamaAdapterError::InvalidExecutable)?;
    let canonical_metadata =
        std::fs::symlink_metadata(&canonical).map_err(|_| OllamaAdapterError::InvalidExecutable)?;
    if !canonical.is_absolute()
        || !canonical.ends_with(Path::new(POWERSHELL_SUFFIX))
        || !canonical_metadata.is_file()
        || canonical_metadata.file_type().is_symlink()
    {
        return Err(OllamaAdapterError::InvalidExecutable);
    }
    Ok(canonical)
}

#[cfg(not(windows))]
fn resolve_system_powershell() -> Result<PathBuf, OllamaAdapterError> {
    let _ = POWERSHELL_SUFFIX;
    Err(OllamaAdapterError::InvalidExecutable)
}

fn validate_authenticode_output(
    stdout: &[u8],
    stderr: &[u8],
    success: bool,
) -> Result<(), OllamaAdapterError> {
    if !success || !stderr.is_empty() {
        return Err(OllamaAdapterError::ExecutableUntrusted);
    }
    let decision: AuthenticodeDecision =
        serde_json::from_slice(stdout).map_err(|_| OllamaAdapterError::ExecutableUntrusted)?;
    if decision.signature_valid && decision.publisher_valid {
        Ok(())
    } else {
        Err(OllamaAdapterError::ExecutableUntrusted)
    }
}

async fn probe_client_version(
    executable: &ValidatedExecutable,
    endpoint: &ValidatedEndpoint,
    limits: ProcessLimits,
) -> Result<semver::Version, OllamaAdapterError> {
    let mut command = Command::new(executable.path());
    command
        .arg(VERSION_ARGUMENT)
        .env(OLLAMA_HOST_ENV, endpoint.child_environment_value())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|_| OllamaAdapterError::ProcessStartFailed)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(OllamaAdapterError::ProcessControlFailed)?;
    let stderr = child
        .stderr
        .take()
        .ok_or(OllamaAdapterError::ProcessControlFailed)?;
    let collected = collect_bounded_output(
        stdout,
        stderr,
        async {
            child
                .wait()
                .await
                .map_err(|_| OllamaAdapterError::ProcessControlFailed)
        },
        limits,
    )
    .await;
    let (stdout, stderr, status) = match collected {
        Ok(collected) => collected,
        Err(error) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(error);
        }
    };
    if !status.success() {
        return Err(OllamaAdapterError::ProcessControlFailed);
    }

    parse_client_version_output(&stdout, &stderr)
}

async fn collect_bounded_output<Stdout, Stderr, Wait, Status>(
    stdout: Stdout,
    stderr: Stderr,
    wait: Wait,
    limits: ProcessLimits,
) -> Result<(Vec<u8>, Vec<u8>, Status), OllamaAdapterError>
where
    Stdout: AsyncRead + Unpin,
    Stderr: AsyncRead + Unpin,
    Wait: Future<Output = Result<Status, OllamaAdapterError>>,
{
    time::timeout(limits.timeout, async {
        tokio::try_join!(
            read_bounded(stdout, limits.max_output_bytes),
            read_bounded(stderr, limits.max_output_bytes),
            wait,
        )
    })
    .await
    .map_err(|_| OllamaAdapterError::TimedOut)?
}

async fn read_bounded<R: AsyncRead + Unpin>(
    reader: R,
    limit: usize,
) -> Result<Vec<u8>, OllamaAdapterError> {
    let mut output = Vec::new();
    reader
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut output)
        .await
        .map_err(|_| OllamaAdapterError::ProcessControlFailed)?;
    if output.len() > limit {
        return Err(OllamaAdapterError::ProcessOutputTooLarge);
    }
    Ok(output)
}

fn parse_client_version_output(
    stdout: &[u8],
    stderr: &[u8],
) -> Result<semver::Version, OllamaAdapterError> {
    let stdout = std::str::from_utf8(stdout).map_err(|_| OllamaAdapterError::InvalidResponse)?;
    let stderr = std::str::from_utf8(stderr).map_err(|_| OllamaAdapterError::InvalidResponse)?;
    let lines: Vec<&str> = stdout.lines().chain(stderr.lines()).collect();

    let version = lines
        .iter()
        .find_map(|line| line.trim().strip_prefix(CLIENT_VERSION_PREFIX))
        .or_else(|| {
            lines
                .iter()
                .find_map(|line| line.trim().strip_prefix(SERVER_VERSION_PREFIX))
        })
        .ok_or(OllamaAdapterError::InvalidResponse)?;
    parse_version(version)
}

#[cfg(test)]
mod tests {
    use std::{
        fs, future,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        task::{Context, Poll},
    };

    use tokio::io::ReadBuf;

    use crate::discovery::{ExecutableLocator, FixedExecutableLocator};

    use super::*;

    struct FakeIdentityVerifier {
        result: Result<(), OllamaAdapterError>,
        calls: AtomicUsize,
    }

    impl FakeIdentityVerifier {
        fn new(result: Result<(), OllamaAdapterError>) -> Self {
            Self {
                result,
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl ExecutableIdentityVerifier for FakeIdentityVerifier {
        fn verify<'a>(
            &'a self,
            _executable: &'a ValidatedExecutable,
            _limits: ProcessLimits,
        ) -> ProcessFuture<'a, ()> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            let result = self.result.clone();
            Box::pin(async move { result })
        }
    }

    struct PendingReader {
        dropped: Arc<AtomicBool>,
    }

    impl AsyncRead for PendingReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            _buffer: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Pending
        }
    }

    impl Drop for PendingReader {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::Release);
        }
    }

    fn fixture_executable() -> (tempfile::TempDir, ValidatedExecutable) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("ollama.exe");
        fs::write(&path, b"not an executable").expect("fixture writes");
        let locator = FixedExecutableLocator::from_path(&path);
        let executable = locator
            .locate()
            .expect("fixture inspection succeeds")
            .expect("fixture executable exists");
        (directory, executable)
    }

    #[test]
    fn publisher_policy_accepts_documented_punctuation_and_case_variants_only() {
        for accepted in ["Ollama Inc.", "Ollama, Inc.", "OLLAMA INC", "ollama-inc"] {
            assert_eq!(
                normalize_publisher_name(accepted).as_deref(),
                Some(TRUSTED_PUBLISHER_KEY),
                "{accepted}"
            );
        }
        for rejected in [
            "Ollama Incorporated",
            "Not Ollama Inc.",
            "Ollama Inc. Test",
            "Ollama\u{00a0}Inc.",
            "",
        ] {
            assert_ne!(
                normalize_publisher_name(rejected).as_deref(),
                Some(TRUSTED_PUBLISHER_KEY),
                "{rejected}"
            );
        }
    }

    #[test]
    fn authenticode_command_uses_fixed_script_and_separate_target_argument() {
        #[cfg(windows)]
        let powershell = Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
        #[cfg(not(windows))]
        let powershell = Path::new("/Windows/System32/WindowsPowerShell/v1.0/powershell.exe");
        let target = Path::new(r"C:\Program Files\Ol;la$()#ma\ollama.exe");
        let command = authenticode_command(powershell, target).expect("path encodes");
        let command = command.as_std();
        let arguments = command.get_args().collect::<Vec<_>>();

        assert_eq!(command.get_program(), powershell.as_os_str());
        assert_eq!(command.get_current_dir(), powershell.parent());
        let expected_module_path = powershell
            .parent()
            .expect("PowerShell has a parent")
            .join("Modules");
        assert!(command.get_envs().any(|(key, value)| {
            key == "PSModulePath" && value == Some(expected_module_path.as_os_str())
        }));
        let target_token = arguments
            .last()
            .and_then(|argument| argument.to_str())
            .expect("encoded target is ASCII");
        assert!(target_token.starts_with('x'));
        assert!(
            target_token[1..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        );
        assert!(!target_token.contains([';', '$', '(', ')', '#', ' ']));
        let script = arguments
            .get(arguments.len() - 2)
            .and_then(|argument| argument.to_str())
            .expect("fixed script is UTF-8");
        assert!(!script.contains(target.to_string_lossy().as_ref()));
        assert!(script.contains("ValidatePattern('^x[0-9A-F]+$')"));
        assert!(script.contains("Microsoft.PowerShell.Core\\Import-Module"));
        assert!(script.contains("Microsoft.PowerShell.Security.psd1"));
        assert!(script.contains("Microsoft.PowerShell.Security\\Get-AuthenticodeSignature"));
        assert!(!script.contains("ConvertTo-Json"));
        assert!(!script.contains("subject ="));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_powershell_treats_adversarial_executable_path_as_data() {
        let directory = tempfile::tempdir().expect("temporary directory");
        fs::write(directory.path().join("probe"), b"unsigned").expect("prefix fixture writes");
        let target = directory
            .path()
            .join("probe;Write-Output INJECTED;$();#.exe");
        fs::write(&target, b"unsigned").expect("adversarial fixture writes");
        let target = target.canonicalize().expect("fixture canonicalizes");
        let powershell = resolve_system_powershell().expect("fixed system PowerShell resolves");
        let output = authenticode_command(&powershell, &target)
            .expect("path encodes")
            .output()
            .await
            .expect("fixed system PowerShell starts");

        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            br#"{"signature_valid":false,"publisher_valid":false}"#
        );
    }

    #[test]
    fn valid_authenticode_boolean_payload_is_accepted() {
        assert_eq!(
            validate_authenticode_output(
                br#"{"signature_valid":true,"publisher_valid":true}"#,
                b"",
                true,
            ),
            Ok(())
        );
    }

    #[test]
    fn wrong_publisher_and_unsigned_payloads_are_rejected() {
        for payload in [
            br#"{"signature_valid":true,"publisher_valid":false}"#.as_slice(),
            br#"{"signature_valid":false,"publisher_valid":true}"#.as_slice(),
            br#"{"signature_valid":false,"publisher_valid":false}"#.as_slice(),
        ] {
            assert_eq!(
                validate_authenticode_output(payload, b"", true),
                Err(OllamaAdapterError::ExecutableUntrusted)
            );
        }
    }

    #[test]
    fn malformed_or_diagnostic_authenticode_output_is_rejected() {
        for (stdout, stderr, success) in [
            (b"not-json".as_slice(), b"".as_slice(), true),
            (
                br#"{"signature_valid":"true","publisher_valid":true}"#.as_slice(),
                b"".as_slice(),
                true,
            ),
            (
                br#"{"signature_valid":true,"publisher_valid":true,"subject":"private"}"#
                    .as_slice(),
                b"".as_slice(),
                true,
            ),
            (
                br#"{"signature_valid":true,"publisher_valid":true}"#.as_slice(),
                b"diagnostic".as_slice(),
                true,
            ),
            (
                br#"{"signature_valid":true,"publisher_valid":true}"#.as_slice(),
                b"".as_slice(),
                false,
            ),
        ] {
            assert_eq!(
                validate_authenticode_output(stdout, stderr, success),
                Err(OllamaAdapterError::ExecutableUntrusted)
            );
        }
    }

    #[tokio::test]
    async fn authenticode_output_is_rejected_at_the_fixed_bound() {
        let result = collect_bounded_output(
            tokio::io::repeat(b'x').take((AUTHENTICODE_OUTPUT_LIMIT + 1) as u64),
            tokio::io::empty(),
            future::ready(Ok(())),
            ProcessLimits {
                timeout: Duration::from_secs(1),
                max_output_bytes: AUTHENTICODE_OUTPUT_LIMIT,
            },
        )
        .await;

        assert_eq!(result, Err(OllamaAdapterError::ProcessOutputTooLarge));
    }

    #[tokio::test]
    async fn identity_failure_blocks_version_and_serve_before_provider_spawn() {
        let (_directory, executable) = fixture_executable();
        let endpoint = ValidatedEndpoint::documented_default();
        for error in [
            OllamaAdapterError::ExecutableUntrusted,
            OllamaAdapterError::InvalidExecutable,
            OllamaAdapterError::ExecutableIo {
                operation: "verify Authenticode",
                kind: std::io::ErrorKind::NotFound,
            },
            OllamaAdapterError::ProcessOutputTooLarge,
            OllamaAdapterError::TimedOut,
        ] {
            let verifier = Arc::new(FakeIdentityVerifier::new(Err(error.clone())));
            let processes = TokioOwnedProcessControl::with_identity_verifier(verifier.clone());
            let limits = ProcessLimits {
                timeout: Duration::from_secs(1),
                max_output_bytes: 64,
            };

            assert_eq!(
                processes
                    .client_version(&executable, &endpoint, limits)
                    .await,
                Err(error.clone())
            );
            assert_eq!(
                processes.start_owned(&executable, &endpoint).await,
                Err(error)
            );
            assert_eq!(verifier.calls.load(Ordering::Acquire), 2);
            assert_eq!(processes.owned_status().await, Ok(OwnedProcessStatus::None));
        }
    }

    #[test]
    fn client_version_wins_when_server_and_binary_differ() {
        let parsed = parse_client_version_output(
            b"ollama version is 0.12.6\nWarning: client version is 0.13.0\n",
            b"",
        )
        .unwrap();

        assert_eq!(parsed, semver::Version::new(0, 13, 0));
    }

    #[test]
    fn unreachable_server_output_still_reports_binary_version() {
        let parsed = parse_client_version_output(
            b"Warning: could not connect to a running Ollama instance\nWarning: client version is 0.13.0\n",
            b"",
        )
        .unwrap();

        assert_eq!(parsed, semver::Version::new(0, 13, 0));
    }

    #[test]
    fn arbitrary_command_output_is_not_accepted() {
        assert_eq!(
            parse_client_version_output(b"version 0.13.0", b""),
            Err(OllamaAdapterError::InvalidResponse)
        );
    }

    #[tokio::test]
    async fn timed_out_output_collection_drops_pending_pipe_readers() {
        let stdout_dropped = Arc::new(AtomicBool::new(false));
        let stderr_dropped = Arc::new(AtomicBool::new(false));
        let result = collect_bounded_output(
            PendingReader {
                dropped: Arc::clone(&stdout_dropped),
            },
            PendingReader {
                dropped: Arc::clone(&stderr_dropped),
            },
            future::pending::<Result<(), OllamaAdapterError>>(),
            ProcessLimits {
                timeout: Duration::from_millis(10),
                max_output_bytes: 16,
            },
        )
        .await;

        assert_eq!(result, Err(OllamaAdapterError::TimedOut));
        assert!(stdout_dropped.load(Ordering::Acquire));
        assert!(stderr_dropped.load(Ordering::Acquire));
    }
}
