use std::{
    env,
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

#[cfg(windows)]
use std::path::{Component, Prefix};

#[cfg(test)]
use std::sync::Arc;

use crate::error::OllamaAdapterError;

const EXECUTABLE_NAME: &str = "ollama.exe";
const DOCUMENTED_INSTALL_SUFFIX: [&str; 3] = ["Programs", "Ollama", EXECUTABLE_NAME];

/// Canonical executable evidence. Discovery does not grant management ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedExecutable {
    path: PathBuf,
}

impl ValidatedExecutable {
    #[must_use]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Injectable discovery boundary used by deterministic adapter tests.
pub(crate) trait ExecutableLocator: Send + Sync {
    fn locate(&self) -> Result<Option<ValidatedExecutable>, OllamaAdapterError>;
}

/// Non-privileged Windows executable discovery using the documented current-user location.
#[derive(Clone, Debug, Default)]
pub(crate) struct WindowsExecutableLocator {
    explicit_path: Option<PathBuf>,
}

impl WindowsExecutableLocator {
    #[must_use]
    pub(crate) fn new(explicit_path: Option<PathBuf>) -> Self {
        Self { explicit_path }
    }

    fn current_inputs(&self) -> DiscoveryInputs {
        DiscoveryInputs {
            explicit_path: self.explicit_path.clone(),
            local_app_data: env::var_os("LOCALAPPDATA"),
        }
    }
}

impl ExecutableLocator for WindowsExecutableLocator {
    fn locate(&self) -> Result<Option<ValidatedExecutable>, OllamaAdapterError> {
        locate_from(&self.current_inputs())
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DiscoveryInputs {
    pub(crate) explicit_path: Option<PathBuf>,
    pub(crate) local_app_data: Option<OsString>,
}

pub(crate) fn locate_from(
    inputs: &DiscoveryInputs,
) -> Result<Option<ValidatedExecutable>, OllamaAdapterError> {
    if let Some(explicit) = &inputs.explicit_path {
        if !is_local_absolute(explicit) {
            return Err(OllamaAdapterError::InvalidExecutable);
        }
        return validate_candidate(explicit).map(Some);
    }

    let Some(local_app_data) = &inputs.local_app_data else {
        return Ok(None);
    };
    let approved_root = PathBuf::from(local_app_data);
    if !is_local_absolute(&approved_root) {
        return Err(OllamaAdapterError::InvalidExecutable);
    }
    let mut candidate = approved_root.clone();
    candidate.extend(DOCUMENTED_INSTALL_SUFFIX);
    match fs::symlink_metadata(&candidate) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            let validated = validate_candidate(&candidate)?;
            let approved_root = approved_root
                .canonicalize()
                .map_err(|error| OllamaAdapterError::executable_io("canonicalize", &error))?;
            if !validated.path.starts_with(approved_root) {
                return Err(OllamaAdapterError::InvalidExecutable);
            }
            Ok(Some(validated))
        }
        Ok(_) => Err(OllamaAdapterError::InvalidExecutable),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(OllamaAdapterError::executable_io("metadata", &error)),
    }
}

fn is_local_absolute(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    #[cfg(windows)]
    {
        matches!(
            path.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        )
    }
    #[cfg(not(windows))]
    {
        true
    }
}

fn validate_candidate(candidate: &Path) -> Result<ValidatedExecutable, OllamaAdapterError> {
    if !candidate
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(EXECUTABLE_NAME))
    {
        return Err(OllamaAdapterError::InvalidExecutable);
    }

    let candidate_metadata = fs::symlink_metadata(candidate)
        .map_err(|error| OllamaAdapterError::executable_io("metadata", &error))?;
    if !candidate_metadata.is_file() || candidate_metadata.file_type().is_symlink() {
        return Err(OllamaAdapterError::InvalidExecutable);
    }

    let path = candidate
        .canonicalize()
        .map_err(|error| OllamaAdapterError::executable_io("canonicalize", &error))?;
    let metadata = path
        .metadata()
        .map_err(|error| OllamaAdapterError::executable_io("metadata", &error))?;
    if !metadata.is_file()
        || !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(EXECUTABLE_NAME))
    {
        return Err(OllamaAdapterError::InvalidExecutable);
    }

    Ok(ValidatedExecutable { path })
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct FixedExecutableLocator {
    result: Result<Option<ValidatedExecutable>, OllamaAdapterError>,
}

#[cfg(test)]
impl FixedExecutableLocator {
    pub(crate) fn new(
        result: Result<Option<ValidatedExecutable>, OllamaAdapterError>,
    ) -> Arc<Self> {
        Arc::new(Self { result })
    }

    pub(crate) fn from_path(path: &Path) -> Arc<Self> {
        Self::new(validate_candidate(path).map(Some))
    }
}

#[cfg(test)]
impl ExecutableLocator for FixedExecutableLocator {
    fn locate(&self) -> Result<Option<ValidatedExecutable>, OllamaAdapterError> {
        self.result.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn create_executable(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"test fixture").unwrap();
    }

    #[test]
    fn explicit_candidate_is_canonicalized_and_wins() {
        let root = tempdir().unwrap();
        let executable = root.path().join(EXECUTABLE_NAME);
        create_executable(&executable);

        let located = locate_from(&DiscoveryInputs {
            explicit_path: Some(executable.clone()),
            ..DiscoveryInputs::default()
        })
        .unwrap()
        .unwrap();

        assert_eq!(located.path(), executable.canonicalize().unwrap());
    }

    #[test]
    fn documented_install_is_discovered() {
        let root = tempdir().unwrap();
        let documented = root
            .path()
            .join("Programs")
            .join("Ollama")
            .join(EXECUTABLE_NAME);
        create_executable(&documented);

        let located = locate_from(&DiscoveryInputs {
            explicit_path: None,
            local_app_data: Some(root.path().as_os_str().to_owned()),
        })
        .unwrap()
        .unwrap();

        assert_eq!(located.path(), documented.canonicalize().unwrap());
    }

    #[test]
    fn missing_candidates_are_not_installed_evidence() {
        let root = tempdir().unwrap();
        let located = locate_from(&DiscoveryInputs {
            explicit_path: None,
            local_app_data: Some(root.path().as_os_str().to_owned()),
        })
        .unwrap();

        assert_eq!(located, None);
    }

    #[test]
    fn explicit_non_provider_file_fails_closed() {
        let root = tempdir().unwrap();
        let executable = root.path().join("other.exe");
        create_executable(&executable);

        assert_eq!(
            locate_from(&DiscoveryInputs {
                explicit_path: Some(executable),
                ..DiscoveryInputs::default()
            }),
            Err(OllamaAdapterError::InvalidExecutable)
        );
    }

    #[test]
    fn relative_discovery_roots_and_explicit_paths_fail_closed() {
        assert_eq!(
            locate_from(&DiscoveryInputs {
                explicit_path: Some(PathBuf::from("ollama.exe")),
                local_app_data: None,
            }),
            Err(OllamaAdapterError::InvalidExecutable)
        );
        assert_eq!(
            locate_from(&DiscoveryInputs {
                explicit_path: None,
                local_app_data: Some(OsString::from(".")),
            }),
            Err(OllamaAdapterError::InvalidExecutable)
        );
    }
}
