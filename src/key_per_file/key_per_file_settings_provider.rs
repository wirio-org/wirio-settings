use arc_swap::ArcSwap;
use pyo3::prelude::*;
use pyo3::{exceptions::PyRuntimeError, types::PyDict};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use tokio::fs;

use crate::core::{PathProvider, PythonSettingsProvider, SettingLookup, SettingsProvider};

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct KeyPerFileSettingsProvider {
    data: ArcSwap<Py<PyDict>>,
    path_provider: PathProvider,
}

#[pymethods]
impl KeyPerFileSettingsProvider {
    #[pyo3(signature = () -> "dict[str, str | None]")]
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        SettingsProvider::data(self, py)
    }

    fn try_get(&self, py: Python<'_>, key: &str) -> PyResult<SettingLookup> {
        SettingsProvider::try_get(self, py, key)
    }

    pub fn load(&self, py: Python<'_>) -> PyResult<()> {
        SettingsProvider::load(self, py)
    }
}

impl KeyPerFileSettingsProvider {
    pub fn new(py: Python<'_>, path_provider: PathProvider) -> Self {
        Self {
            data: ArcSwap::from_pointee(PyDict::new(py).unbind()),
            path_provider,
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
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        let data = self.data.load();
        data.clone_ref(py)
    }

    async fn reload(&self) -> PyResult<()> {
        if !self.path_provider.try_is_path_available().await? {
            return Ok(());
        }

        let mut parsed_data: BTreeMap<String, Option<String>> = BTreeMap::new();
        let mut directory_entries =
            fs::read_dir(self.path_provider.path())
                .await
                .map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to read the directory '{}': {}",
                        self.path_provider.path().display(),
                        error
                    ))
                })?;

        while let Some(directory_entry) = directory_entries.next_entry().await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to read the directory entry in '{}': {}",
                self.path_provider.path().display(),
                error
            ))
        })? {
            let directory_entry_path = directory_entry.path();
            let file_type = directory_entry.file_type().await.map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to inspect the entry '{}': {}",
                    directory_entry_path.display(),
                    error
                ))
            })?;

            if file_type.is_dir() {
                continue;
            }

            if file_type.is_symlink() {
                let entry_metadata =
                    fs::metadata(&directory_entry_path).await.map_err(|error| {
                        PyRuntimeError::new_err(format!(
                            "Failed to inspect the entry '{}': {}",
                            directory_entry_path.display(),
                            error
                        ))
                    })?;

                if entry_metadata.is_dir() {
                    continue;
                }
            }

            let file_name = directory_entry.file_name().to_string_lossy().into_owned();

            let file_content =
                fs::read_to_string(&directory_entry_path)
                    .await
                    .map_err(|error| {
                        PyRuntimeError::new_err(format!(
                            "Failed to read the entry '{}': {}",
                            directory_entry_path.display(),
                            error
                        ))
                    })?;

            parsed_data.insert(file_name, Some(Self::trim_new_line(file_content)));
        }

        Self::normalize_keys(&mut parsed_data);
        let data = Python::attach(|py| Self::create_data(py, parsed_data))?;
        self.data.store(Arc::new(data));
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
    use crate::core::{PathProvider, SettingsProvider};
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;
    use std::{collections::BTreeMap, path::PathBuf};
    use tempfile::tempdir;
    use tokio::fs;

    fn assert_data(
        provider: &KeyPerFileSettingsProvider,
        expected_data: &BTreeMap<String, Option<String>>,
    ) {
        Python::attach(|py| {
            let data = SettingsProvider::data(provider, py);
            let actual_data = data
                .bind(py)
                .extract::<BTreeMap<String, Option<String>>>()
                .unwrap();
            assert_eq!(&actual_data, expected_data);
        });
    }

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
        let provider = Python::attach(|py| {
            KeyPerFileSettingsProvider::new(
                py,
                PathProvider::from_directory(temporary_directory.path().to_str().unwrap(), false)
                    .unwrap(),
            )
        });

        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(
            &provider,
            &BTreeMap::from([
                (String::from("app_name"), Some(String::from("wirio"))),
                (
                    String::from("database_password"),
                    Some(String::from("secret")),
                ),
                (
                    String::from("logging__log_level__default"),
                    Some(String::from("WARNING")),
                ),
            ]),
        );
    }

    #[tokio::test]
    async fn test_return_empty_data_when_optional_directory_is_missing() {
        let temporary_directory = tempdir().unwrap();
        let missing_directory_path = temporary_directory.path().join("missing");
        let provider = Python::attach(|py| {
            KeyPerFileSettingsProvider::new(
                py,
                PathProvider::from_directory(missing_directory_path.to_str().unwrap(), true)
                    .unwrap(),
            )
        });

        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(&provider, &BTreeMap::new());
    }

    #[tokio::test]
    async fn test_fail_when_required_directory_is_missing() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let missing_directory_path = temporary_directory.path().join("missing");
        let provider = Python::attach(|py| {
            KeyPerFileSettingsProvider::new(
                py,
                PathProvider::from_directory(missing_directory_path.to_str().unwrap(), false)
                    .unwrap(),
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();

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
        let provider = Python::attach(|py| {
            KeyPerFileSettingsProvider::new(
                py,
                PathProvider::from_directory(file_path.to_str().unwrap(), false).unwrap(),
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();

        let error_message = error.to_string();
        assert_eq!(
            error_message,
            format!("RuntimeError: '{}' is not a directory", file_path.display())
        );
    }

    #[tokio::test]
    async fn test_fail_when_checking_directory_existence_with_invalid_path() {
        Python::initialize();

        let invalid_directory_path = PathBuf::from("/\0invalid");
        let provider = Python::attach(|py| {
            KeyPerFileSettingsProvider::new(
                py,
                PathProvider::from_directory(invalid_directory_path.to_str().unwrap(), false)
                    .unwrap(),
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();

        let error_message = error.to_string();
        assert!(error_message.contains("RuntimeError: Failed to inspect"));
    }

    #[test]
    fn test_display_returns_type_name() {
        Python::initialize();

        let directory_path = std::env::current_dir().unwrap();
        let display = Python::attach(|py| {
            KeyPerFileSettingsProvider::new(
                py,
                PathProvider::from_directory(directory_path.to_str().unwrap(), false).unwrap(),
            )
            .to_string()
        });

        assert_eq!(display, "KeyPerFileSettingsProvider");
    }

    #[test]
    fn test_trim_trailing_line_feed() {
        let value = KeyPerFileSettingsProvider::trim_new_line(String::from("value\n"));

        assert_eq!(value, "value");
    }

    #[test]
    fn test_trim_trailing_carriage_return_and_line_feed() {
        let value = KeyPerFileSettingsProvider::trim_new_line(String::from("value\r\n"));

        assert_eq!(value, "value");
    }

    #[test]
    fn test_preserve_value_without_trailing_new_line() {
        let value = KeyPerFileSettingsProvider::trim_new_line(String::from("value"));

        assert_eq!(value, "value");
    }

    #[test]
    fn test_preserve_additional_trailing_new_line() {
        let value = KeyPerFileSettingsProvider::trim_new_line(String::from("value\n\n"));

        assert_eq!(value, "value\n");
    }
}
