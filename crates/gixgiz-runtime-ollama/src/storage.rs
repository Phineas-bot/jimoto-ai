use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Mutex,
};

use crate::error::OllamaAdapterError;

const OLLAMA_MODELS_ENV: &str = "OLLAMA_MODELS";

pub(crate) trait ModelStorageProbe: Send + Sync {
    fn available_bytes(&self) -> Result<u64, OllamaAdapterError>;
}

#[derive(Debug)]
pub(crate) struct FsModelStorageProbe {
    requested_root: Option<PathBuf>,
    state: Mutex<StorageProbeState>,
}

#[derive(Debug, Default)]
struct StorageProbeState {
    canonical_anchor: Option<PathBuf>,
    exact_root_observed: bool,
}

impl Default for FsModelStorageProbe {
    fn default() -> Self {
        Self {
            requested_root: provider_models_path().ok(),
            state: Mutex::new(StorageProbeState::default()),
        }
    }
}

impl ModelStorageProbe for FsModelStorageProbe {
    fn available_bytes(&self) -> Result<u64, OllamaAdapterError> {
        let requested = self
            .requested_root
            .clone()
            .ok_or(OllamaAdapterError::StorageUnavailable)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| OllamaAdapterError::StorageUnavailable)?;
        let (existing, exact_root_observed) =
            resolve_probe_path(requested, state.exact_root_observed)?;
        let canonical =
            fs::canonicalize(existing).map_err(|_| OllamaAdapterError::StorageUnavailable)?;
        if !is_allowed_models_path(&canonical) {
            return Err(OllamaAdapterError::StorageUnavailable);
        }
        validate_and_update_anchor(&canonical, exact_root_observed, &mut state)?;
        let available =
            fs4::available_space(canonical).map_err(|_| OllamaAdapterError::StorageUnavailable)?;
        if exact_root_observed {
            state.exact_root_observed = true;
        }
        Ok(available)
    }
}

fn provider_models_path() -> Result<PathBuf, OllamaAdapterError> {
    let configured = env::var_os(OLLAMA_MODELS_ENV).filter(|value| !value.is_empty());
    let path = if let Some(configured) = configured {
        PathBuf::from(configured)
    } else {
        let home = env::var_os("USERPROFILE")
            .or_else(|| env::var_os("HOME"))
            .filter(|value| !value.is_empty())
            .ok_or(OllamaAdapterError::StorageUnavailable)?;
        PathBuf::from(home).join(".ollama").join("models")
    };
    if !is_allowed_models_path(&path) {
        return Err(OllamaAdapterError::StorageUnavailable);
    }
    Ok(path)
}

#[cfg(windows)]
fn is_allowed_models_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};

    path.is_absolute()
        && matches!(
            path.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        )
}

#[cfg(not(windows))]
fn is_allowed_models_path(path: &Path) -> bool {
    path.is_absolute()
}

fn resolve_probe_path(
    path: PathBuf,
    exact_root_observed: bool,
) -> Result<(PathBuf, bool), OllamaAdapterError> {
    match inspect_path(&path) {
        Ok(ExistingPathKind::Directory) => Ok((path, true)),
        Ok(ExistingPathKind::Other) => Err(OllamaAdapterError::StorageUnavailable),
        Err(error) if error.kind() == ErrorKind::NotFound && !exact_root_observed => {
            nearest_existing_path(path).map(|existing| (existing, false))
        }
        Err(_) => Err(OllamaAdapterError::StorageUnavailable),
    }
}

fn validate_and_update_anchor(
    canonical: &Path,
    exact_root_observed: bool,
    state: &mut StorageProbeState,
) -> Result<(), OllamaAdapterError> {
    match state.canonical_anchor.as_deref() {
        None => state.canonical_anchor = Some(canonical.to_path_buf()),
        Some(anchor) if state.exact_root_observed => {
            if canonical != anchor {
                return Err(OllamaAdapterError::StorageUnavailable);
            }
        }
        Some(anchor) if exact_root_observed => {
            if !canonical.starts_with(anchor) {
                return Err(OllamaAdapterError::StorageUnavailable);
            }
            state.canonical_anchor = Some(canonical.to_path_buf());
        }
        Some(anchor) if canonical != anchor => {
            return Err(OllamaAdapterError::StorageUnavailable);
        }
        Some(_) => {}
    }
    Ok(())
}

fn nearest_existing_path(path: PathBuf) -> Result<PathBuf, OllamaAdapterError> {
    nearest_existing_path_with(path, inspect_path)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExistingPathKind {
    Directory,
    Other,
}

fn inspect_path(path: &Path) -> Result<ExistingPathKind, std::io::Error> {
    fs::metadata(path).map(|metadata| {
        if metadata.is_dir() {
            ExistingPathKind::Directory
        } else {
            ExistingPathKind::Other
        }
    })
}

fn nearest_existing_path_with(
    mut path: PathBuf,
    mut inspect: impl FnMut(&Path) -> Result<ExistingPathKind, std::io::Error>,
) -> Result<PathBuf, OllamaAdapterError> {
    loop {
        match inspect(&path) {
            Ok(ExistingPathKind::Directory) => return Ok(path),
            Ok(ExistingPathKind::Other) => return Err(OllamaAdapterError::StorageUnavailable),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err(OllamaAdapterError::StorageUnavailable),
        }
        if !path.pop() {
            return Err(OllamaAdapterError::StorageUnavailable);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        cell::Cell,
        ffi::OsString,
        io::{Error, ErrorKind},
        sync::Mutex,
    };

    use super::*;

    pub(crate) struct FakeModelStorageProbe {
        result: Mutex<Result<u64, OllamaAdapterError>>,
    }

    impl FakeModelStorageProbe {
        pub(crate) fn new(result: Result<u64, OllamaAdapterError>) -> Self {
            Self {
                result: Mutex::new(result),
            }
        }
    }

    impl ModelStorageProbe for FakeModelStorageProbe {
        fn available_bytes(&self) -> Result<u64, OllamaAdapterError> {
            self.result.lock().expect("fake storage lock").clone()
        }
    }

    #[test]
    fn nearest_existing_path_never_creates_provider_storage() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let nested = directory.path().join("not-created").join("models");

        let existing = nearest_existing_path(nested).expect("ancestor exists");

        assert_eq!(existing, directory.path());
        assert!(!directory.path().join("not-created").exists());
    }

    #[test]
    fn a_previously_observed_root_cannot_disappear_into_its_parent() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let model_root = directory.path().join("models");

        assert_eq!(
            resolve_probe_path(model_root.clone(), false),
            Ok((directory.path().to_path_buf(), false))
        );
        assert_eq!(
            resolve_probe_path(model_root.clone(), true),
            Err(OllamaAdapterError::StorageUnavailable)
        );

        fs::create_dir(&model_root).expect("model root");
        assert_eq!(
            resolve_probe_path(model_root.clone(), false),
            Ok((model_root, true))
        );
    }

    #[test]
    fn canonical_storage_anchor_cannot_change_after_approval() {
        let mut state = StorageProbeState::default();
        let original_parent = Path::new("approved-root");
        let exact_root = original_parent.join("models");

        validate_and_update_anchor(original_parent, false, &mut state)
            .expect("initial missing-leaf anchor");
        validate_and_update_anchor(&exact_root, true, &mut state)
            .expect("exact root beneath anchor");
        state.exact_root_observed = true;

        assert_eq!(
            validate_and_update_anchor(Path::new("other-root/models"), true, &mut state),
            Err(OllamaAdapterError::StorageUnavailable)
        );
    }

    #[cfg(windows)]
    #[test]
    fn network_and_device_roots_are_not_approved_as_local_model_storage() {
        assert!(!is_allowed_models_path(Path::new(r"\\server\share\models")));
        assert!(!is_allowed_models_path(Path::new(r"\\.\C:\models")));
        assert!(!is_allowed_models_path(Path::new(r"C:models")));
        assert!(is_allowed_models_path(Path::new(r"C:\models")));
    }

    #[test]
    fn configured_file_is_not_treated_as_a_storage_directory() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let file = directory.path().join("models-file");
        fs::write(&file, b"not a directory").expect("fixture file");

        assert_eq!(
            nearest_existing_path(file),
            Err(OllamaAdapterError::StorageUnavailable)
        );
    }

    #[test]
    fn inaccessible_path_fails_without_climbing_to_a_parent() {
        let calls = Cell::new(0_u8);
        let result = nearest_existing_path_with(PathBuf::from("unreadable/models"), |_| {
            calls.set(calls.get().saturating_add(1));
            Err(Error::from(ErrorKind::PermissionDenied))
        });

        assert_eq!(result, Err(OllamaAdapterError::StorageUnavailable));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn empty_values_are_not_accepted_as_paths() {
        assert!(OsString::new().is_empty());
    }
}
