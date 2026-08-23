//! User-selected storage locations for runtimes, models, and downloads.
//!
//! Rust is the authority: a client supplies a *candidate* path, never a
//! setting. Every candidate is normalized and checked before it can be stored,
//! and a rejected candidate never becomes durable state.
//!
//! Changing a location applies to **new** content only. Anything already stored
//! stays where it is, and the caller is told so explicitly rather than
//! discovering split content later.

use std::path::{Component, Path, PathBuf};

use gixgiz_contracts::{
    STORAGE_LOCATION_SCHEMA_VERSION, SetupDestinationCategory, StorageLocationKind,
    StorageLocationRejection, StorageLocationSetting, StorageLocationValidation,
    StorageLocationWarning, StorageLocationWarningCode,
};
use gixgiz_persistence::{PersistedStorageLocation, Persistence, StorageLocationRepository};

use crate::CoreError;

/// Reads free space for the drive containing a path.
///
/// Abstracted so validation is testable without touching real volumes.
pub trait FreeSpaceProbe: Send + Sync {
    /// Returns free bytes, or `None` when the drive cannot be inspected.
    fn free_bytes(&self, path: &Path) -> Option<u64>;
}

/// Validates and stores user-selected storage locations.
pub struct StorageLocationService {
    repository: StorageLocationRepository,
    probe: std::sync::Arc<dyn FreeSpaceProbe>,
}

impl StorageLocationService {
    /// Composes the service over Rust-owned persistence.
    #[must_use]
    pub fn with_persistence(
        persistence: &Persistence,
        probe: std::sync::Arc<dyn FreeSpaceProbe>,
    ) -> Self {
        Self {
            repository: persistence.storage_locations(),
            probe,
        }
    }

    /// Checks a candidate path without storing it.
    #[must_use]
    pub fn validate(&self, kind: StorageLocationKind, path: &str) -> StorageLocationValidation {
        let required = kind.required_bytes();
        let normalized = match normalize(path) {
            Ok(normalized) => normalized,
            Err(rejection) => return rejected(rejection, None, required),
        };

        // Probe the nearest existing ancestor: the chosen directory itself may
        // not exist yet, but its drive still has to be usable.
        let probe_target = nearest_existing(&normalized);
        let Some(free) = self.probe.free_bytes(&probe_target) else {
            return rejected(StorageLocationRejection::Unavailable, None, required);
        };
        if free < required {
            return rejected(
                StorageLocationRejection::InsufficientSpace,
                Some(free),
                required,
            );
        }
        StorageLocationValidation {
            accepted: true,
            rejection: None,
            free_bytes: Some(free),
            required_bytes: required,
        }
    }

    /// Returns the stored location for one kind, or `None` for the default.
    pub fn resolved(&self, kind: StorageLocationKind) -> Result<Option<PathBuf>, CoreError> {
        let Some(stored) = self
            .repository
            .get(kind_key(kind))
            .map_err(|_| CoreError::SetupPersistenceUnavailable)?
        else {
            return Ok(None);
        };
        Ok(stored.path.map(PathBuf::from))
    }

    /// Returns every setting, filling in defaults for kinds never chosen.
    pub fn settings(
        &self,
        external_runtime: bool,
    ) -> Result<Vec<StorageLocationSetting>, CoreError> {
        let stored = self
            .repository
            .list()
            .map_err(|_| CoreError::SetupPersistenceUnavailable)?;
        Ok([
            StorageLocationKind::Runtime,
            StorageLocationKind::Models,
            StorageLocationKind::Staging,
        ]
        .into_iter()
        .map(|kind| {
            let key = kind_key(kind);
            let found = stored.iter().find(|item| item.kind == key);
            StorageLocationSetting {
                schema_version: STORAGE_LOCATION_SCHEMA_VERSION,
                kind,
                category: found.map_or(default_category(kind), |item| {
                    parse_category(&item.category)
                }),
                path: found.and_then(|item| item.path.clone()),
                is_default: found.is_none(),
                // Pointing a runtime GixGiz does not own at a new model store is
                // an ownership change, so it needs its own explicit acceptance.
                requires_external_consent: external_runtime && kind == StorageLocationKind::Models,
            }
        })
        .collect())
    }

    /// Stores or clears one location.
    ///
    /// Returns the stored setting plus every warning the user should see.
    pub fn set(
        &self,
        kind: StorageLocationKind,
        path: Option<&str>,
        external_runtime: bool,
        accepted_external_reconfiguration: bool,
        now_unix_ms: i64,
    ) -> Result<SetOutcome, CoreError> {
        if kind == StorageLocationKind::Unknown {
            return Ok(SetOutcome {
                setting: None,
                validation: rejected(
                    StorageLocationRejection::InvalidPath,
                    None,
                    kind.required_bytes(),
                ),
                warnings: Vec::new(),
            });
        }

        let needs_consent = external_runtime && kind == StorageLocationKind::Models;
        if needs_consent && !accepted_external_reconfiguration {
            // Refuse rather than silently reconfigure a runtime owned elsewhere.
            return Ok(SetOutcome {
                setting: None,
                validation: rejected(
                    StorageLocationRejection::Unavailable,
                    None,
                    kind.required_bytes(),
                ),
                warnings: vec![StorageLocationWarning {
                    code: StorageLocationWarningCode::ExternalRuntimeReconfigured,
                    message: "This runtime was installed outside GixGiz. Accept the change \
                              explicitly before its model location is altered."
                        .to_owned(),
                }],
            });
        }

        let Some(path) = path else {
            self.repository
                .clear(kind_key(kind))
                .map_err(|_| CoreError::SetupPersistenceUnavailable)?;
            return Ok(SetOutcome {
                setting: Some(StorageLocationSetting {
                    schema_version: STORAGE_LOCATION_SCHEMA_VERSION,
                    kind,
                    category: default_category(kind),
                    path: None,
                    is_default: true,
                    requires_external_consent: needs_consent,
                }),
                validation: StorageLocationValidation {
                    accepted: true,
                    rejection: None,
                    free_bytes: None,
                    required_bytes: kind.required_bytes(),
                },
                warnings: warnings_for(kind, needs_consent),
            });
        };

        let validation = self.validate(kind, path);
        if !validation.accepted {
            return Ok(SetOutcome {
                setting: None,
                validation,
                warnings: Vec::new(),
            });
        }

        let normalized = match normalize(path) {
            Ok(normalized) => normalized,
            Err(rejection) => {
                return Ok(SetOutcome {
                    setting: None,
                    validation: rejected(rejection, None, kind.required_bytes()),
                    warnings: Vec::new(),
                });
            }
        };
        let stored_path = normalized.to_string_lossy().into_owned();

        self.repository
            .upsert(&PersistedStorageLocation {
                kind: kind_key(kind).to_owned(),
                category: "user_selected".to_owned(),
                path: Some(stored_path.clone()),
                external_consent_at_unix_ms: needs_consent.then_some(now_unix_ms),
                updated_at_unix_ms: now_unix_ms,
            })
            .map_err(|_| CoreError::SetupPersistenceUnavailable)?;

        Ok(SetOutcome {
            setting: Some(StorageLocationSetting {
                schema_version: STORAGE_LOCATION_SCHEMA_VERSION,
                kind,
                category: SetupDestinationCategory::UserSelected,
                path: Some(stored_path),
                is_default: false,
                requires_external_consent: needs_consent,
            }),
            validation,
            warnings: warnings_for(kind, needs_consent),
        })
    }
}

/// Result of storing or clearing one location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetOutcome {
    /// Stored setting when the change was accepted.
    pub setting: Option<StorageLocationSetting>,
    /// Validation outcome for the requested path.
    pub validation: StorageLocationValidation,
    /// Safe warnings the user should see.
    pub warnings: Vec<StorageLocationWarning>,
}

fn warnings_for(kind: StorageLocationKind, external: bool) -> Vec<StorageLocationWarning> {
    let mut warnings = vec![StorageLocationWarning {
        code: StorageLocationWarningCode::ExistingContentRemains,
        message: "Anything already downloaded stays in its current location and is not moved."
            .to_owned(),
    }];
    if matches!(
        kind,
        StorageLocationKind::Models | StorageLocationKind::Runtime
    ) {
        warnings.push(StorageLocationWarning {
            code: StorageLocationWarningCode::RestartRequired,
            message: "The local runtime must restart before this location takes effect.".to_owned(),
        });
    }
    if external {
        warnings.push(StorageLocationWarning {
            code: StorageLocationWarningCode::ExternalRuntimeReconfigured,
            message: "A runtime installed outside GixGiz was pointed at a new model location. \
                      Models it already had remain where they were and will not be listed."
                .to_owned(),
        });
    }
    warnings
}

/// Normalizes and screens a candidate path.
///
/// Rejects anything that is not a plain absolute local directory path, so a
/// network share, device path, or traversal cannot become a storage root.
fn normalize(path: &str) -> Result<PathBuf, StorageLocationRejection> {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.len() > gixgiz_contracts::MAX_STORAGE_PATH_BYTES
        || trimmed.contains('\0')
    {
        return Err(StorageLocationRejection::InvalidPath);
    }
    // UNC and device namespaces are screened before any filesystem call.
    if trimmed.starts_with("\\\\") || trimmed.starts_with("//") {
        return Err(StorageLocationRejection::NetworkPath);
    }
    if trimmed.starts_with("\\\\.\\") || trimmed.starts_with("\\\\?\\") {
        return Err(StorageLocationRejection::DevicePath);
    }
    let candidate = PathBuf::from(trimmed);
    if !candidate.is_absolute() {
        return Err(StorageLocationRejection::NotAbsolute);
    }
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(StorageLocationRejection::InvalidPath);
    }
    if candidate.is_file() {
        return Err(StorageLocationRejection::NotADirectory);
    }
    Ok(candidate)
}

/// Returns the nearest existing ancestor, for probing a not-yet-created path.
fn nearest_existing(path: &Path) -> PathBuf {
    let mut current = path;
    loop {
        if current.exists() {
            return current.to_path_buf();
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return path.to_path_buf(),
        }
    }
}

const fn rejected(
    rejection: StorageLocationRejection,
    free_bytes: Option<u64>,
    required_bytes: u64,
) -> StorageLocationValidation {
    StorageLocationValidation {
        accepted: false,
        rejection: Some(rejection),
        free_bytes,
        required_bytes,
    }
}

const fn kind_key(kind: StorageLocationKind) -> &'static str {
    match kind {
        StorageLocationKind::Runtime => "runtime",
        StorageLocationKind::Models => "models",
        StorageLocationKind::Staging => "staging",
        // A kind this build does not understand has no storage key; callers
        // reject it before it can reach persistence.
        _ => "unknown",
    }
}

const fn default_category(kind: StorageLocationKind) -> SetupDestinationCategory {
    match kind {
        // Without a choice, the provider decides where models live.
        StorageLocationKind::Models => SetupDestinationCategory::ProviderManaged,
        _ => SetupDestinationCategory::ApplicationData,
    }
}

fn parse_category(value: &str) -> SetupDestinationCategory {
    match value {
        "user_selected" => SetupDestinationCategory::UserSelected,
        "application_data" => SetupDestinationCategory::ApplicationData,
        "provider_managed" => SetupDestinationCategory::ProviderManaged,
        _ => SetupDestinationCategory::Unknown,
    }
}

/// Windows free-space probe using the same fixed inbox PowerShell the hardware
/// scanner uses.
///
/// The path is passed as a separate argument and never interpolated into the
/// script, so an adversarial path stays data.
#[derive(Clone, Debug, Default)]
pub struct WindowsFreeSpaceProbe;

const FREE_SPACE_SCRIPT: &str = r#"& {
    param([Parameter(Mandatory = $true, Position = 0)][string] $Target)
    $ErrorActionPreference = 'Stop'
    $root = [System.IO.Path]::GetPathRoot($Target)
    if (-not $root) { throw 'no root' }
    $deviceId = $root.TrimEnd([char]92)
    $disk = Microsoft.PowerShell.Management\Get-CimInstance `
        -ClassName Win32_LogicalDisk -Filter "DeviceID='$deviceId'" -Property FreeSpace
    if ($null -eq $disk) { throw 'no drive' }
    [Console]::Out.Write([string][long]$disk.FreeSpace)
}"#;

impl FreeSpaceProbe for WindowsFreeSpaceProbe {
    fn free_bytes(&self, path: &Path) -> Option<u64> {
        #[cfg(not(windows))]
        {
            let _ = path;
            let _ = FREE_SPACE_SCRIPT;
            return None;
        }
        #[cfg(windows)]
        {
            let powershell = std::path::PathBuf::from(
                r"\\?\GLOBALROOT\SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe",
            );
            let output = std::process::Command::new(&powershell)
                .arg("-NoLogo")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg(FREE_SPACE_SCRIPT)
                .arg(path.as_os_str())
                .stdin(std::process::Stdio::null())
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            String::from_utf8_lossy(&output.stdout).trim().parse().ok()
        }
    }
}

#[cfg(test)]
mod tests;
