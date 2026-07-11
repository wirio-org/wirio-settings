use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use tokio::fs;

use crate::core::SettingsProvider;

#[pyclass(str)]
pub struct PythonKeyPerFileSettingsProvider {
    provider: KeyPerFileSettingsProvider,
}

#[pymethods]
impl PythonKeyPerFileSettingsProvider {
    #[new]
    fn new(directory_path: &str, optional: bool) -> Self {
        Self {
            provider: KeyPerFileSettingsProvider::new(directory_path, optional),
        }
    }

    #[getter]
    fn data(&self) -> &BTreeMap<String, Option<String>> {
        &self.provider.data
    }

    pub fn load(&mut self) -> PyResult<()> {
        let runtime = tokio::runtime::Runtime::new().map_err(|error| {
            PyRuntimeError::new_err(format!("Failed to create Tokio runtime: {error}"))
        })?;

        runtime.block_on(self.provider.load())
    }
}

impl fmt::Display for PythonKeyPerFileSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeyPerFileSettingsProvider")
    }
}

struct KeyPerFileSettingsProvider {
    data: BTreeMap<String, Option<String>>,
    directory_path: PathBuf,
    optional: bool,
}

impl KeyPerFileSettingsProvider {
    /// Adds settings using files from a directory. File names are used as the key, file contents are used as the value.
    fn new(directory_path: &str, optional: bool) -> Self {
        Self {
            data: BTreeMap::new(),
            directory_path: PathBuf::from(directory_path),
            optional,
        }
    }

    fn trim_new_line(value: String) -> String {
        if let Some(trimmed_value) = value.strip_suffix("\r\n") {
            return trimmed_value.to_string();
        }

        if let Some(trimmed_value) = value.strip_suffix('\n') {
            return trimmed_value.to_string();
        }

        value
    }
}

impl SettingsProvider for KeyPerFileSettingsProvider {
    async fn load(&mut self) -> PyResult<()> {
        let directory_exists = fs::try_exists(&self.directory_path).await.unwrap_or(false);

        if !directory_exists {
            if self.optional {
                return Ok(());
            }

            return Err(PyRuntimeError::new_err(format!(
                "'{}' does not exist",
                self.directory_path.display()
            )));
        }

        let directory_metadata = fs::metadata(&self.directory_path).await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to inspect '{}': {}",
                self.directory_path.display(),
                error
            ))
        })?;

        if !directory_metadata.is_dir() {
            return Err(PyRuntimeError::new_err(format!(
                "'{}' is not a directory",
                self.directory_path.display()
            )));
        }

        let mut parsed_data: BTreeMap<String, Option<String>> = BTreeMap::new();
        let mut directory_entries = fs::read_dir(&self.directory_path).await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to read the directory '{}': {}",
                self.directory_path.display(),
                error
            ))
        })?;

        while let Some(directory_entry) = directory_entries.next_entry().await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to read the directory entry in '{}': {}",
                self.directory_path.display(),
                error
            ))
        })? {
            let file_type = directory_entry.file_type().await.map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to inspect the entry '{}': {}",
                    directory_entry.path().display(),
                    error
                ))
            })?;

            if file_type.is_dir() {
                continue;
            }

            let file_name = directory_entry.file_name().to_string_lossy().into_owned();

            let file_content =
                fs::read_to_string(directory_entry.path())
                    .await
                    .map_err(|error| {
                        PyRuntimeError::new_err(format!(
                            "Failed to read the entry '{}': {}",
                            directory_entry.path().display(),
                            error
                        ))
                    })?;

            parsed_data.insert(file_name, Some(Self::trim_new_line(file_content)));
        }

        self.normalize_keys(&mut parsed_data);
        self.data = parsed_data;
        Ok(())
    }
}

impl fmt::Display for KeyPerFileSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}

#[cfg(test)]
mod tests {
    use super::KeyPerFileSettingsProvider;
    use crate::core::SettingsProvider;
    use pyo3::Python;
    use std::collections::BTreeMap;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn test_load_values_from_directory_files() {
        let temporary_directory = tempdir().unwrap();
        fs::write(temporary_directory.path().join("app_name"), "wirio")
            .await
            .unwrap();
        fs::write(
            temporary_directory
                .path()
                .join("logging__log_level__default"),
            "WARNING",
        )
        .await
        .unwrap();
        fs::write(
            temporary_directory.path().join("database_password"),
            "secret",
        )
        .await
        .unwrap();
        let mut provider =
            KeyPerFileSettingsProvider::new(temporary_directory.path().to_str().unwrap(), false);

        provider.load().await.unwrap();

        assert_eq!(
            provider.data,
            BTreeMap::from([
                (String::from("app_name"), Some(String::from("wirio"))),
                (
                    String::from("database_password"),
                    Some(String::from("secret")),
                ),
                (
                    String::from("logging__log_level__default"),
                    Some(String::from("WARNING")),
                ),
            ])
        );
    }

    #[tokio::test]
    async fn test_return_empty_data_when_optional_directory_is_missing() {
        let temporary_directory = tempdir().unwrap();
        let missing_directory_path = temporary_directory.path().join("missing");
        let mut provider =
            KeyPerFileSettingsProvider::new(missing_directory_path.to_str().unwrap(), true);

        provider.load().await.unwrap();

        assert_eq!(provider.data, BTreeMap::new());
    }

    #[tokio::test]
    async fn test_fail_when_required_directory_is_missing() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let missing_directory_path = temporary_directory.path().join("missing");
        let mut provider =
            KeyPerFileSettingsProvider::new(missing_directory_path.to_str().unwrap(), false);

        let error = provider.load().await.unwrap_err();

        let error_message = error.to_string();
        assert_eq!(
            error_message,
            format!(
                "RuntimeError: '{}' does not exist",
                missing_directory_path.display()
            )
        );
    }

    #[tokio::test]
    async fn test_fail_when_path_points_to_file() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("not-a-directory");
        fs::write(&file_path, "value").await.unwrap();
        let mut provider = KeyPerFileSettingsProvider::new(file_path.to_str().unwrap(), false);

        let error = provider.load().await.unwrap_err();

        let error_message = error.to_string();
        assert_eq!(
            error_message,
            format!("RuntimeError: '{}' is not a directory", file_path.display())
        );
    }
}
