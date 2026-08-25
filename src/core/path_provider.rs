use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::core::PathWatcher;

#[derive(Debug, Clone)]
pub struct PathProvider {
    path: PathBuf,
    is_directory: bool,
    optional: bool,
}

impl PathProvider {
    /// Creates a `PathProvider` for a file path.
    /// When the path is rooted (e.g., starting with `/` on Linux), it's used as is, otherwise, it's resolved against the content root path if provided, or the current working directory.
    ///
    /// The `path` argument can be:
    /// - A file name (for example, `settings.yaml`).
    /// - A relative path (for example, `config/settings.yaml` or `../settings.yaml`).
    /// - An absolute path (for example, `/tmp/settings.yaml`).
    pub fn from_file(
        content_root_path: Option<&str>,
        path: &str,
        optional: bool,
    ) -> PyResult<Self> {
        let path = PathBuf::from(path);

        let path = if path.has_root() {
            path
        } else {
            let content_root_path = if let Some(content_root_path) = content_root_path {
                let final_content_root_path = PathBuf::from(content_root_path);

                if !final_content_root_path.has_root() {
                    return Err(PyValueError::new_err(format!(
                        "When file path is not rooted, content root path ('{}') must be rooted",
                        path.display()
                    )));
                }

                final_content_root_path
            } else {
                std::env::current_dir().map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to determine the current working directory: {error}"
                    ))
                })?
            };

            content_root_path.join(path)
        };

        Ok(Self {
            path,
            is_directory: false,
            optional,
        })
    }

    /// Creates a `PathProvider` for a directory path. The path must be rooted (e.g., starting with `/` on Linux).
    pub fn from_directory(path: &str, optional: bool) -> PyResult<Self> {
        let path = PathBuf::from(path);

        if !path.has_root() {
            return Err(PyValueError::new_err(format!(
                "Directory path '{}' must be rooted",
                path.display()
            )));
        }

        Ok(Self {
            path,
            is_directory: true,
            optional,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn create_watcher(&self) -> PathWatcher {
        PathWatcher::new(self.path.clone())
    }

    /// Checks if the path exists and is of the expected type (file or directory). If the path is optional and doesn't exist, it returns `false`. If the path is required and doesn't exist, it returns an error.
    pub async fn try_is_path_available(&self) -> PyResult<bool> {
        let path_exists = fs::try_exists(&self.path).await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to inspect '{}': {}",
                self.path.display(),
                error
            ))
        })?;

        if !path_exists {
            if self.optional {
                return Ok(false);
            }

            return Err(PyRuntimeError::new_err(format!(
                "Path '{}' does not exist",
                self.path.display()
            )));
        }

        let metadata = fs::metadata(&self.path).await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to inspect '{}': {}",
                self.path.display(),
                error
            ))
        })?;
        let is_file = !self.is_directory;

        if is_file && !metadata.is_file() {
            return Err(PyRuntimeError::new_err(format!(
                "'{}' is not a file",
                self.path.display()
            )));
        }

        if self.is_directory && !metadata.is_dir() {
            return Err(PyRuntimeError::new_err(format!(
                "'{}' is not a directory",
                self.path.display()
            )));
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pyo3::Python;

    use crate::core::PathProvider;

    #[test]
    fn test_return_absolute_path_ignoring_content_root_path() {
        let absolute_path = PathBuf::from("/tmp/settings.yaml");

        let path_provider = PathProvider::from_file(
            Some("ignored/content/root"),
            absolute_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        assert_eq!(path_provider.path(), absolute_path);
    }

    #[test]
    fn test_resolve_relative_path_using_content_root_path() {
        let expected_path = PathBuf::from("/etc/wirio/config/settings.yaml");

        let path_provider =
            PathProvider::from_file(Some("/etc/wirio"), "config/settings.yaml", false).unwrap();

        assert_eq!(path_provider.path(), expected_path);
    }

    #[test]
    fn test_resolve_file_name_using_content_root_path() {
        let expected_path = PathBuf::from("/etc/wirio/settings.yaml");

        let path_provider =
            PathProvider::from_file(Some("/etc/wirio"), "settings.yaml", false).unwrap();

        assert_eq!(path_provider.path(), expected_path);
    }

    #[test]
    fn test_resolve_relative_path_using_current_directory_when_content_root_is_not_provided() {
        let relative_path = "config/settings.yaml";
        let current_directory = std::env::current_dir().unwrap();
        let expected_path = current_directory.join(relative_path);

        let path_provider = PathProvider::from_file(None, "config/settings.yaml", false).unwrap();

        assert_eq!(path_provider.path(), expected_path);
    }

    #[test]
    fn test_fail_when_creating_file_path_using_unrooted_content_root_path() {
        Python::initialize();
        let error = PathProvider::from_file(Some("config"), "settings.yaml", false)
            .err()
            .unwrap();

        assert_eq!(
            error.to_string(),
            "ValueError: When file path is not rooted, content root path ('settings.yaml') must be rooted"
        );
    }

    #[test]
    fn test_fail_when_creating_directory_path_using_unrooted_path() {
        Python::initialize();
        let error = PathProvider::from_directory("config", false).err().unwrap();

        assert_eq!(
            error.to_string(),
            "ValueError: Directory path 'config' must be rooted"
        );
    }

    #[tokio::test]
    async fn test_succeed_when_validating_missing_optional_path() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let missing_path = temporary_directory.path().join("missing.yaml");
        let path_provider =
            PathProvider::from_file(None, missing_path.to_str().unwrap(), true).unwrap();

        let is_path_available = path_provider.try_is_path_available().await.unwrap();

        assert!(!is_path_available);
    }

    #[tokio::test]
    async fn test_fail_when_validating_missing_required_path() {
        Python::initialize();
        let temporary_directory = tempfile::tempdir().unwrap();
        let missing_path = temporary_directory.path().join("missing.yaml");
        let path_provider =
            PathProvider::from_file(None, missing_path.to_str().unwrap(), false).unwrap();

        let error = path_provider.try_is_path_available().await.unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("RuntimeError: '{}' does not exist", missing_path.display())
        );
    }

    #[tokio::test]
    async fn test_fail_when_validating_directory_path_using_file() {
        Python::initialize();
        let temporary_file = tempfile::NamedTempFile::new().unwrap();
        let path_provider =
            PathProvider::from_directory(temporary_file.path().to_str().unwrap(), false).unwrap();

        let error = path_provider.try_is_path_available().await.unwrap_err();

        assert_eq!(
            error.to_string(),
            format!(
                "RuntimeError: '{}' is not a directory",
                temporary_file.path().display()
            )
        );
    }

    #[test]
    fn test_create_file_path_as_rooted_path_using_current_working_directory() {
        let current_working_directory = std::env::current_dir().unwrap();
        let path_provider = PathProvider::from_file(None, "settings.yaml", false).unwrap();

        assert!(path_provider.path().has_root());
        assert!(path_provider.path().starts_with(current_working_directory));
    }

    #[test]
    fn test_create_provider_using_directory() {
        let temporary_directory = tempfile::tempdir().unwrap();

        PathProvider::from_directory(temporary_directory.path().to_str().unwrap(), false).unwrap();
    }

    #[tokio::test]
    async fn test_fail_when_validating_file_path_using_directory_path() {
        Python::initialize();
        let temporary_directory = tempfile::tempdir().unwrap();
        let path_provider =
            PathProvider::from_file(None, temporary_directory.path().to_str().unwrap(), false)
                .unwrap();

        let error = path_provider.try_is_path_available().await.unwrap_err();

        assert_eq!(
            error.to_string(),
            format!(
                "RuntimeError: '{}' is not a file",
                temporary_directory.path().display()
            )
        );
    }
}
