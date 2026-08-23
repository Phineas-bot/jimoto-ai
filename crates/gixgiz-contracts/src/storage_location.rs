//! User-selectable storage locations for runtimes, models, and downloads.
//!
//! # Why raw paths appear here
//!
//! Evidence contracts deliberately keep raw filesystem paths off the boundary,
//! because scan results could otherwise leak private structure. A storage
//! location is the exception: the user chose it, is shown it, and must be able
//! to confirm it. The path is therefore carried explicitly, bounded in length,
//! and validated by Rust before it is ever stored or used.
//!
//! Rust remains the authority. A path supplied by a client is a *request*, not
//! a setting: it is canonicalized and checked before acceptance, and a rejected
//! path never becomes durable state.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CorrelationId, RequestId, SetupDestinationCategory};

/// Schema generation for storage-location settings.
pub const STORAGE_LOCATION_SCHEMA_VERSION: u32 = 1;

/// Maximum accepted length of a storage path.
pub const MAX_STORAGE_PATH_BYTES: usize = 240;

/// Which stored artifact a location applies to.
///
/// Each kind is chosen independently, so a large model library can live on one
/// drive while the runtime stays on another.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum StorageLocationKind {
    /// Where a GixGiz-managed runtime is installed.
    Runtime,
    /// Where the runtime keeps downloaded models.
    Models,
    /// Where GixGiz stages downloads before verifying them.
    Staging,
    /// A newer peer supplied an unrecognized kind.
    #[serde(other)]
    Unknown,
}

impl StorageLocationKind {
    /// Returns the conservative free space this kind needs to be usable.
    #[must_use]
    pub const fn required_bytes(self) -> u64 {
        match self {
            // A runtime expands well beyond its compressed installer.
            Self::Runtime => 6 * 1024 * 1024 * 1024,
            // One small model plus headroom; larger models are checked per plan.
            Self::Models => 4 * 1024 * 1024 * 1024,
            // Staging holds one installer artifact at a time.
            Self::Staging => 2 * 1024 * 1024 * 1024,
            Self::Unknown => u64::MAX,
        }
    }
}

/// Why a requested storage path was refused.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum StorageLocationRejection {
    /// The path was empty or exceeded the accepted length.
    InvalidPath,
    /// The path was not absolute, so it could resolve unpredictably.
    NotAbsolute,
    /// A network share cannot host a local runtime or model store.
    NetworkPath,
    /// A device or volume path is not a usable storage root.
    DevicePath,
    /// The path exists but is a file rather than a directory.
    NotADirectory,
    /// The path could not be created or written to.
    NotWritable,
    /// The drive does not have the free space this kind requires.
    InsufficientSpace,
    /// The drive could not be inspected.
    Unavailable,
    /// A newer peer supplied an unrecognized rejection.
    #[serde(other)]
    Unknown,
}

/// Result of checking one requested storage path.
///
/// Returned before anything is stored so the user sees why a path is unusable
/// rather than discovering it during a download.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StorageLocationValidation {
    /// Whether the path may be used for this kind.
    pub accepted: bool,
    /// Why the path was refused, when it was.
    pub rejection: Option<StorageLocationRejection>,
    /// Free bytes observed on the containing drive, when it could be read.
    pub free_bytes: Option<u64>,
    /// Free bytes this kind requires.
    pub required_bytes: u64,
}

/// One durable storage-location setting.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StorageLocationSetting {
    /// Storage-location schema generation.
    pub schema_version: u32,
    /// Which artifact this location applies to.
    pub kind: StorageLocationKind,
    /// Who owns the destination.
    pub category: SetupDestinationCategory,
    /// Absolute path, present only for a user-selected location.
    pub path: Option<String>,
    /// Whether this is the built-in default rather than a user choice.
    pub is_default: bool,
    /// Whether changing this location currently requires extra consent.
    ///
    /// True for model storage when the runtime is externally owned, because
    /// pointing someone else's runtime at a new store is an ownership change.
    pub requires_external_consent: bool,
}

/// Reads current storage-location settings.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StorageLocationsRequest {
    /// Identifier shared with the response.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Current storage-location settings.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StorageLocationsResponse {
    /// One setting per known kind.
    pub settings: Vec<StorageLocationSetting>,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Checks a candidate path without storing it.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ValidateStorageLocationRequest {
    /// Which artifact the path is for.
    pub kind: StorageLocationKind,
    /// Absolute candidate path.
    pub path: String,
    /// Identifier shared with the response.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Result of checking a candidate path.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ValidateStorageLocationResponse {
    /// Validation outcome.
    pub validation: StorageLocationValidation,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Sets or clears one storage location.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetStorageLocationRequest {
    /// Which artifact the location is for.
    pub kind: StorageLocationKind,
    /// Absolute path, or `None` to return to the built-in default.
    pub path: Option<String>,
    /// Explicit acceptance of reconfiguring an externally owned runtime.
    ///
    /// Required only when the setting demands it. Rust refuses the change
    /// without it rather than silently reconfiguring another owner's runtime.
    #[serde(default)]
    pub accept_external_reconfiguration: bool,
    /// Identifier shared with the response.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative setting after the write.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetStorageLocationResponse {
    /// Stored setting when the change was accepted.
    pub setting: Option<StorageLocationSetting>,
    /// Validation outcome for the requested path.
    pub validation: StorageLocationValidation,
    /// Safe warnings the user should see, such as content left behind.
    pub warnings: Vec<StorageLocationWarning>,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Stable warning category for a storage-location change.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum StorageLocationWarningCode {
    /// Content already stored at the previous location stays there.
    ExistingContentRemains,
    /// The runtime must restart before the new location takes effect.
    RestartRequired,
    /// An externally owned runtime was reconfigured with explicit consent.
    ExternalRuntimeReconfigured,
    /// A newer peer supplied an unrecognized warning.
    #[serde(other)]
    Unknown,
}

/// One safe warning about a storage-location change.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StorageLocationWarning {
    /// Stable warning category.
    pub code: StorageLocationWarningCode,
    /// Bounded beginner-readable explanation.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_declares_a_conservative_requirement() {
        assert!(StorageLocationKind::Runtime.required_bytes() > 0);
        assert!(StorageLocationKind::Models.required_bytes() > 0);
        assert!(StorageLocationKind::Staging.required_bytes() > 0);
        // A runtime expands well beyond one staged artifact.
        assert!(
            StorageLocationKind::Runtime.required_bytes()
                > StorageLocationKind::Staging.required_bytes()
        );
    }

    #[test]
    fn an_unknown_kind_can_never_satisfy_its_requirement() {
        // Fails closed: a kind this build does not understand is never usable.
        assert_eq!(StorageLocationKind::Unknown.required_bytes(), u64::MAX);
    }

    #[test]
    fn unknown_peer_values_deserialize_without_failing_closed_types() {
        let kind: StorageLocationKind =
            serde_json::from_str("\"a_future_kind\"").expect("kind tolerates newer peers");
        assert_eq!(kind, StorageLocationKind::Unknown);

        let rejection: StorageLocationRejection =
            serde_json::from_str("\"a_future_reason\"").expect("rejection tolerates newer peers");
        assert_eq!(rejection, StorageLocationRejection::Unknown);
    }

    #[test]
    fn external_consent_defaults_to_absent() {
        let request: SetStorageLocationRequest = serde_json::from_str(
            r#"{"kind":"models","path":null,
                "correlation_id":"00000000-0000-4000-8000-000000000000",
                "request_id":"00000000-0000-4000-8000-000000000001"}"#,
        )
        .expect("request parses without the consent field");

        assert!(!request.accept_external_reconfiguration);
    }
}
