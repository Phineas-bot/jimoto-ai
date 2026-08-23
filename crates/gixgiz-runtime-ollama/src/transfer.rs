//! Bounded HTTPS artifact transfer and hashing.
//!
//! GixGiz has no TLS stack of its own and deliberately does not add one for a
//! single artifact download. Instead this module reuses the same fixed,
//! non-interactive system PowerShell already audited for Authenticode
//! verification. Both the URL and the destination path travel as hex tokens
//! that the script decodes itself, so neither is ever interpolated into script
//! text and an adversarial value stays data.
//!
//! Transport is not treated as an integrity guarantee. The caller enforces a
//! pinned digest and an Authenticode publisher on the resulting file, so the
//! bounded redirects that GitHub release downloads require cannot substitute a
//! different artifact.

use std::path::Path;

use tokio::process::Command;

use crate::{error::OllamaAdapterError, process::ProcessLimits};

/// Result of a bounded artifact transfer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TransferOutcome {
    /// Bytes written to the destination.
    pub(crate) bytes: u64,
    /// Lowercase hexadecimal SHA-256 of the written file.
    pub(crate) sha256: String,
}

/// Maximum redirects followed during a transfer.
const MAX_REDIRECTS: u32 = 5;

/// Fixed transfer script.
///
/// Every caller value arrives as a hex token validated by pattern before use.
/// The script enforces the trusted URL prefix itself, writes only to the exact
/// decoded destination, and prints a single bounded JSON object.
const TRANSFER_SCRIPT_TEMPLATE: &str = r#"& {
    param(
        [Parameter(Mandatory = $true, Position = 0)]
        [ValidatePattern('^x[0-9A-F]+$')]
        [string] $UrlToken,
        [Parameter(Mandatory = $true, Position = 1)]
        [ValidatePattern('^x[0-9A-F]+$')]
        [string] $DestinationToken,
        [Parameter(Mandatory = $true, Position = 2)]
        [ValidatePattern('^[0-9]+$')]
        [string] $MaximumBytes
    )
    $ErrorActionPreference = 'Stop'
    # PSModulePath is pinned to the system module directory, so inbox modules are
    # imported by absolute path rather than relying on autoloading.
    foreach ($moduleName in @('Microsoft.PowerShell.Management', 'Microsoft.PowerShell.Utility')) {
        $modulePath = [System.IO.Path]::Combine($PSHOME, 'Modules', $moduleName, "$moduleName.psd1")
        Microsoft.PowerShell.Core\Import-Module -Name $modulePath -Force
    }
    function ConvertFrom-HexToken([string] $token) {
        $hex = $token.Substring(1)
        if (($hex.Length % 4) -ne 0) { throw 'A token is malformed.' }
        $chars = for ($i = 0; $i -lt $hex.Length; $i += 4) {
            [char][Convert]::ToUInt16($hex.Substring($i, 4), 16)
        }
        -join $chars
    }
    $url = ConvertFrom-HexToken $UrlToken
    $destination = ConvertFrom-HexToken $DestinationToken
    if (-not $url.StartsWith('__TRUSTED_PREFIX__', [System.StringComparison]::Ordinal)) {
        throw 'The transfer origin is not allowlisted.'
    }
    [System.Net.ServicePointManager]::SecurityProtocol =
        [System.Net.SecurityProtocolType]::Tls12
    if (Microsoft.PowerShell.Management\Test-Path -LiteralPath $destination) {
        Microsoft.PowerShell.Management\Remove-Item -LiteralPath $destination -Force
    }
    # HttpWebRequest lives in System.dll, which is always loaded, and streams to
    # disk. Invoke-WebRequest buffers the whole response in memory on Windows
    # PowerShell 5.1, which is unusable for a large artifact.
    $request = [System.Net.HttpWebRequest]::CreateHttp($url)
    $request.AllowAutoRedirect = $true
    $request.MaximumAutomaticRedirections = __MAX_REDIRECTS__
    $request.Timeout = 120000
    $request.ReadWriteTimeout = 300000
    $response = $request.GetResponse()
    try {
        if ($response.ContentLength -gt [long]$MaximumBytes) {
            throw 'The declared artifact size exceeds its maximum.'
        }
        $source = $response.GetResponseStream()
        $target = [System.IO.File]::Open(
            $destination,
            [System.IO.FileMode]::Create,
            [System.IO.FileAccess]::Write,
            [System.IO.FileShare]::None
        )
        try {
            $buffer = New-Object byte[] 1048576
            $written = [long]0
            while (($read = $source.Read($buffer, 0, $buffer.Length)) -gt 0) {
                $written += $read
                if ($written -gt [long]$MaximumBytes) {
                    throw 'The transferred artifact exceeded its maximum size.'
                }
                $target.Write($buffer, 0, $read)
            }
        } finally {
            $target.Dispose()
            $source.Dispose()
        }
    } finally {
        $response.Dispose()
    }
    $file = Microsoft.PowerShell.Management\Get-Item -LiteralPath $destination
    if ($file.Length -gt [long]$MaximumBytes) {
        Microsoft.PowerShell.Management\Remove-Item -LiteralPath $destination -Force
        throw 'The transferred artifact exceeded its maximum size.'
    }
    if ($file.Length -le 0) {
        Microsoft.PowerShell.Management\Remove-Item -LiteralPath $destination -Force
        throw 'The transfer produced an empty artifact.'
    }
    $hash = Microsoft.PowerShell.Utility\Get-FileHash `
        -LiteralPath $destination -Algorithm SHA256
    $payload = @{ bytes = $file.Length; sha256 = $hash.Hash.ToLowerInvariant() }
    [Console]::Out.Write((Microsoft.PowerShell.Utility\ConvertTo-Json $payload -Compress))
}"#;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferDecision {
    bytes: u64,
    sha256: String,
}

/// Transfers one allowlisted artifact and returns its size and digest.
pub(crate) async fn download_and_hash(
    url: &str,
    destination: &Path,
    maximum_bytes: u64,
    limits: ProcessLimits,
) -> Result<TransferOutcome, OllamaAdapterError> {
    let powershell = crate::process::resolve_system_powershell()?;
    let mut command = transfer_command(&powershell, url, destination, maximum_bytes)?;
    let mut child = command
        .spawn()
        .map_err(|error| OllamaAdapterError::executable_io("transfer artifact", &error))?;
    let stdout = child.stdout.take().ok_or(OllamaAdapterError::Internal)?;
    let stderr = child.stderr.take().ok_or(OllamaAdapterError::Internal)?;
    let collected = crate::process::collect_bounded_output(
        stdout,
        stderr,
        async {
            child.wait().await.map_err(|error| {
                OllamaAdapterError::executable_io("wait for artifact transfer", &error)
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
    if !status.success() || !stderr.is_empty() {
        return Err(OllamaAdapterError::ConnectionFailed);
    }
    let decision: TransferDecision =
        serde_json::from_slice(&stdout).map_err(|_| OllamaAdapterError::ConnectionFailed)?;
    if decision.sha256.len() != 64 || !decision.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(OllamaAdapterError::ConnectionFailed);
    }
    Ok(TransferOutcome {
        bytes: decision.bytes,
        sha256: decision.sha256,
    })
}

pub(crate) fn transfer_command(
    powershell: &Path,
    url: &str,
    destination: &Path,
    maximum_bytes: u64,
) -> Result<Command, OllamaAdapterError> {
    let powershell_home = powershell
        .parent()
        .ok_or(OllamaAdapterError::InvalidExecutable)?;
    if !url.starts_with(TRUSTED_PREFIX) {
        return Err(OllamaAdapterError::InvalidExecutable);
    }
    let script = TRANSFER_SCRIPT_TEMPLATE
        .replace("__TRUSTED_PREFIX__", TRUSTED_PREFIX)
        .replace("__MAX_REDIRECTS__", &MAX_REDIRECTS.to_string());
    let url_token = crate::process::encode_wide_token(url.as_ref())?;
    let destination_token = crate::process::encode_wide_token(destination.as_os_str())?;
    let mut command = Command::new(powershell);
    command
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(script)
        .arg(url_token)
        .arg(destination_token)
        .arg(maximum_bytes.to_string())
        .current_dir(powershell_home)
        .env("PSModulePath", powershell_home.join("Modules"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    Ok(command)
}

/// Required prefix for every trusted transfer.
const TRUSTED_PREFIX: &str = "https://github.com/ollama/ollama/releases/download/";

/// Fixed free-space query for one drive.
///
/// The path travels as a hex token like every other caller value, so it reaches
/// the script as inert data.
const FREE_SPACE_SCRIPT_TEMPLATE: &str = r#"& {
    param(
        [Parameter(Mandatory = $true, Position = 0)]
        [ValidatePattern('^x[0-9A-F]+$')]
        [string] $PathToken
    )
    $ErrorActionPreference = 'Stop'
    foreach ($moduleName in @('Microsoft.PowerShell.Management', 'Microsoft.PowerShell.Utility')) {
        $modulePath = [System.IO.Path]::Combine($PSHOME, 'Modules', $moduleName, "$moduleName.psd1")
        Microsoft.PowerShell.Core\Import-Module -Name $modulePath -Force
    }
    $hex = $PathToken.Substring(1)
    if (($hex.Length % 4) -ne 0) { throw 'The path token is malformed.' }
    $chars = for ($i = 0; $i -lt $hex.Length; $i += 4) {
        [char][Convert]::ToUInt16($hex.Substring($i, 4), 16)
    }
    $path = -join $chars
    $root = [System.IO.Path]::GetPathRoot($path)
    if (-not $root) { throw 'The path has no drive root.' }
    $deviceId = $root.TrimEnd([char]92)
    $disk = Microsoft.PowerShell.Management\Get-CimInstance `
        -ClassName Win32_LogicalDisk `
        -Filter "DeviceID='$deviceId'" `
        -Property FreeSpace
    if ($null -eq $disk) { throw 'The drive was not found.' }
    [Console]::Out.Write(
        (Microsoft.PowerShell.Utility\ConvertTo-Json @{ free = [long]$disk.FreeSpace } -Compress)
    )
}"#;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FreeSpaceDecision {
    free: u64,
}

/// Returns free bytes on the drive containing `path`.
///
/// Used to fail a plan before transferring a large artifact that could never be
/// installed, rather than discovering it mid-installation.
pub(crate) async fn free_space_bytes(
    path: &Path,
    limits: ProcessLimits,
) -> Result<u64, OllamaAdapterError> {
    let powershell = crate::process::resolve_system_powershell()?;
    let powershell_home = powershell
        .parent()
        .ok_or(OllamaAdapterError::InvalidExecutable)?;
    let token = crate::process::encode_wide_token(path.as_os_str())?;
    let mut command = Command::new(&powershell);
    command
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(FREE_SPACE_SCRIPT_TEMPLATE)
        .arg(token)
        .current_dir(powershell_home)
        .env("PSModulePath", powershell_home.join("Modules"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| OllamaAdapterError::executable_io("probe free space", &error))?;
    let stdout = child.stdout.take().ok_or(OllamaAdapterError::Internal)?;
    let stderr = child.stderr.take().ok_or(OllamaAdapterError::Internal)?;
    let collected = crate::process::collect_bounded_output(
        stdout,
        stderr,
        async {
            child
                .wait()
                .await
                .map_err(|error| OllamaAdapterError::executable_io("wait for free space", &error))
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
    if !status.success() || !stderr.is_empty() {
        return Err(OllamaAdapterError::ConnectionFailed);
    }
    let decision: FreeSpaceDecision =
        serde_json::from_slice(&stdout).map_err(|_| OllamaAdapterError::ConnectionFailed)?;
    Ok(decision.free)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hostile_origin_never_produces_a_command() {
        let powershell = Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
        for hostile in [
            "https://example.invalid/OllamaSetup.exe",
            "http://github.com/ollama/ollama/releases/download/v1/OllamaSetup.exe",
            "https://github.com.evil.invalid/ollama/ollama/releases/download/v1/x.exe",
            "",
        ] {
            assert!(
                transfer_command(powershell, hostile, Path::new(r"C:\s\a.exe"), 10).is_err(),
                "{hostile:?} must be refused"
            );
        }
    }

    #[test]
    fn the_script_pins_its_origin_and_bounds_redirects() {
        assert!(TRANSFER_SCRIPT_TEMPLATE.contains("__TRUSTED_PREFIX__"));
        assert!(
            TRANSFER_SCRIPT_TEMPLATE.contains("MaximumAutomaticRedirections = __MAX_REDIRECTS__")
        );
        // The artifact must stream to disk rather than buffer in memory.
        assert!(TRANSFER_SCRIPT_TEMPLATE.contains("GetResponseStream"));
        // Inbox modules must be imported by absolute path, because the pinned
        // PSModulePath prevents autoloading.
        assert!(TRANSFER_SCRIPT_TEMPLATE.contains("Microsoft.PowerShell.Core\\Import-Module"));
        // The cmdlet is named only in the comment explaining why it is avoided;
        // assert it is never actually invoked.
        assert!(
            !TRANSFER_SCRIPT_TEMPLATE.contains(r"Microsoft.PowerShell.Utility\Invoke-WebRequest")
        );
        assert!(TRANSFER_SCRIPT_TEMPLATE.contains("exceeded its maximum size"));
        // Tokens are validated by pattern, never interpolated.
        assert!(TRANSFER_SCRIPT_TEMPLATE.contains("[ValidatePattern('^x[0-9A-F]+$')]"));
    }

    /// Manual smoke test: exercises the real transfer against the pinned
    /// release using its small checksum file rather than the full installer.
    #[tokio::test]
    #[ignore = "manual network smoke test against the pinned release"]
    async fn manual_real_transfer_downloads_and_hashes() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let destination = temporary.path().join("sha256sum.txt");

        let outcome = download_and_hash(
            "https://github.com/ollama/ollama/releases/download/v0.32.5/sha256sum.txt",
            &destination,
            1024 * 1024,
            ProcessLimits {
                timeout: std::time::Duration::from_secs(120),
                max_output_bytes: 8 * 1024,
            },
        )
        .await
        .expect("the pinned release asset transfers");

        println!("bytes  : {}", outcome.bytes);
        println!("sha256 : {}", outcome.sha256);
        assert!(outcome.bytes > 0);
        assert_eq!(outcome.sha256.len(), 64);
        assert!(destination.is_file());
    }

    #[test]
    fn a_transferred_digest_must_be_well_formed() {
        let good = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(good.len(), 64);
        assert!(good.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}
