use arc_swap::ArcSwap;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::{
    PathProvider, PathWatcher, PythonSettingsProvider, SerdeParser, SettingLookup, SettingsProvider,
};

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct YamlFileSettingsProvider {
    data: Arc<ArcSwap<Py<PyDict>>>,
    path_provider: PathProvider,
    reload_on_change: bool,
    path_watcher: Mutex<Option<PathWatcher>>,
}

#[pymethods]
impl YamlFileSettingsProvider {
    #[pyo3(signature = () -> "dict[str, str | None]")]
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        SettingsProvider::data(self, py)
    }

    fn try_get(&self, py: Python<'_>, key: &str) -> PyResult<SettingLookup> {
        SettingsProvider::try_get(self, py, key)
    }

    pub fn load(&self, py: Python<'_>) -> PyResult<()> {
        SettingsProvider::load(self, py)?;
        self.watch_file(py, self.reload_on_change)
    }
}

impl YamlFileSettingsProvider {
    pub fn new(py: Python<'_>, path_provider: PathProvider, reload_on_change: bool) -> Self {
        Self {
            data: Arc::new(ArcSwap::from_pointee(PyDict::new(py).unbind())),
            path_provider,
            reload_on_change,
            path_watcher: Mutex::new(None),
        }
    }

    async fn read_yaml_file(path: &Path) -> PyResult<String> {
        tokio::fs::read_to_string(path).await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to read YAML settings file '{}': {}",
                path.display(),
                error
            ))
        })
    }

    fn watch_file(&self, py: Python<'_>, reload_on_change: bool) -> PyResult<()> {
        if !reload_on_change {
            return Ok(());
        }

        py.detach(|| {
            let data = Arc::clone(&self.data);
            let path_provider = self.path_provider.clone();
            let mut path_watcher = self.path_provider.create_watcher();

            path_watcher
                .watch(move || {
                    let data = Arc::clone(&data);
                    let path_provider = path_provider.clone();

                    async move {
                        // Ignore errors during watched reloads
                        let _ = Self::reload_settings(&data, &path_provider).await;
                    }
                })
                .map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to watch YAML settings file '{}': {}",
                        self.path_provider.path().display(),
                        error
                    ))
                })?;

            self.path_watcher.blocking_lock().replace(path_watcher);
            Ok(())
        })
    }

    async fn reload_settings(
        data: &ArcSwap<Py<PyDict>>,
        path_provider: &PathProvider,
    ) -> PyResult<()> {
        if !path_provider.try_is_path_available().await? {
            return Ok(());
        }

        let path = path_provider.path();
        let raw_yaml = Self::read_yaml_file(path).await?;

        if raw_yaml.trim().is_empty() {
            let new_data = Python::attach(|py| PyDict::new(py).unbind());
            data.store(Arc::new(new_data));
            return Ok(());
        }

        let parsed_yaml: Value = serde_saphyr::from_str(&raw_yaml).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Could not parse YAML file '{}': {}",
                path.display(),
                error
            ))
        })?;

        if parsed_yaml.is_null() {
            let new_data = Python::attach(|py| PyDict::new(py).unbind());
            data.store(Arc::new(new_data));
            return Ok(());
        }

        let yaml_object = parsed_yaml
            .as_object()
            .ok_or_else(|| PyRuntimeError::new_err("Could not parse the YAML file"))?;

        let mut parsed_data = SerdeParser::new().parse(yaml_object)?;
        Self::normalize_keys(&mut parsed_data);
        let new_data = Python::attach(|py| Self::create_data(py, parsed_data))?;
        data.store(Arc::new(new_data));
        Ok(())
    }
}

impl SettingsProvider for YamlFileSettingsProvider {
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        let data = self.data.load();
        data.clone_ref(py)
    }

    async fn reload(&self) -> PyResult<()> {
        Self::reload_settings(&self.data, &self.path_provider).await
    }
}

impl fmt::Display for YamlFileSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}

#[cfg(test)]
mod tests {
    use super::YamlFileSettingsProvider;
    use crate::core::{PathProvider, SettingsProvider};
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn assert_data(
        provider: &YamlFileSettingsProvider,
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
    async fn test_load_values_from_yaml_file() {
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        tokio::fs::write(
            &file_path,
            "
appName: wirio
port: 8080
price: 19.99
intList:
  - 1
  - 2
  - 3
stringList:
  - alpha
  - beta
fieldWithoutValue:
logging:
  enabled: true
  logLevel:

    default: warning

    notes: null
",
        )
        .await
        .unwrap();

        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });
        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(
            &provider,
            &BTreeMap::from([
                (String::from("app_name"), Some(String::from("wirio"))),
                (String::from("port"), Some(String::from("8080"))),
                (String::from("price"), Some(String::from("19.99"))),
                (String::from("int_list.0"), Some(String::from("1"))),
                (String::from("int_list.1"), Some(String::from("2"))),
                (String::from("int_list.2"), Some(String::from("3"))),
                (String::from("string_list.0"), Some(String::from("alpha"))),
                (String::from("string_list.1"), Some(String::from("beta"))),
                (String::from("field_without_value"), None),
                (String::from("logging.enabled"), Some(String::from("true"))),
                (
                    String::from("logging.log_level.default"),
                    Some(String::from("warning")),
                ),
                (String::from("logging.log_level.notes"), None),
            ]),
        );
    }

    #[tokio::test]
    async fn test_ignore_comments() {
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        tokio::fs::write(
            &file_path,
            "# This is a comment
appName: wirio # This is an inline comment
# Another comment
port: 8080
",
        )
        .await
        .unwrap();

        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });
        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(
            &provider,
            &BTreeMap::from([
                (String::from("app_name"), Some(String::from("wirio"))),
                (String::from("port"), Some(String::from("8080"))),
            ]),
        );
    }

    #[tokio::test]
    async fn test_return_empty_data_when_yaml_file_is_empty() {
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        tokio::fs::write(&file_path, "").await.unwrap();

        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });
        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(&provider, &BTreeMap::new());
    }

    #[tokio::test]
    async fn test_return_empty_data_when_yaml_file_has_only_comments() {
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        tokio::fs::write(
            &file_path,
            "# This is a comment
# Another comment
",
        )
        .await
        .unwrap();

        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });
        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(&provider, &BTreeMap::new());
    }

    #[tokio::test]
    async fn test_return_empty_data_when_optional_file_is_missing() {
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("missing.yaml");

        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), true).unwrap(),
                false,
            )
        });
        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(&provider, &BTreeMap::new());
    }

    #[tokio::test]
    async fn test_fail_when_required_file_is_missing() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("missing.yaml");

        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();
        let error_message = error.to_string();

        assert_eq!(
            error_message,
            format!(
                "RuntimeError: Path '{}' does not exist",
                file_path.display()
            )
        );
    }

    #[tokio::test]
    async fn test_fail_when_path_points_to_directory() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, temporary_directory.path().to_str().unwrap(), false)
                    .unwrap(),
                false,
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();
        let error_message = error.to_string();

        assert_eq!(
            error_message,
            format!(
                "RuntimeError: '{}' is not a file",
                temporary_directory.path().display()
            )
        );
    }

    #[tokio::test]
    async fn test_fail_when_yaml_has_invalid_syntax() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        tokio::fs::write(&file_path, "appName: [wirio").await.unwrap();

        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();
        let error_message = error.to_string();

        assert!(error_message.contains("Could not parse"));
        assert!(error_message.contains("YAML"));
    }

    #[tokio::test]
    async fn test_fail_when_yaml_root_value_is_not_object() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        tokio::fs::write(&file_path, "- wirio\n- config").await.unwrap();

        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();
        let error_message = error.to_string();

        assert!(error_message.contains("Could not parse the YAML file"));
    }

    #[test]
    fn test_display_returns_type_name() {
        Python::initialize();

        let display = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, "settings.yaml", false).unwrap(),
                false,
            )
            .to_string()
        });

        assert_eq!(display, "YamlFileSettingsProvider");
    }

    #[tokio::test]
    async fn test_fail_when_checking_file_existence_with_invalid_path() {
        Python::initialize();

        let invalid_file_path = PathBuf::from("\0invalid.yaml");
        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, invalid_file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();
        let error_message = error.to_string();

        assert!(error_message.contains("RuntimeError: Failed to inspect"));
    }

    #[tokio::test]
    async fn test_set_none_and_empty_for_empty_structures() {
        let expected_parsed_yaml = BTreeMap::from([
            (String::from("section"), None),
            (String::from("nested_section.section"), None),
            (String::from("items"), Some(String::new())),
            (String::from("nested_items.items"), Some(String::new())),
        ]);
        let raw_yaml =
            "section: {}\nnested_section:\n  section: {}\nitems: []\nnested_items:\n  items: []";
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        tokio::fs::write(&file_path, raw_yaml).await.unwrap();
        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });

        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(&provider, &expected_parsed_yaml);
    }

    #[test]
    fn test_reload_values_when_yaml_file_is_updated() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        runtime
            .block_on(tokio::fs::write(&file_path, "value: initial"))
            .unwrap();
        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                true,
            )
        });
        Python::attach(|py| provider.load(py)).unwrap();

        let actual_value = runtime.block_on(async {
            tokio::fs::write(&file_path, "value: updated").await.unwrap();

            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let value = Python::attach(|py| {
                        let data = SettingsProvider::data(&provider, py);
                        data.bind(py)
                            .get_item("value")
                            .unwrap()
                            .extract::<String>()
                            .unwrap()
                    });

                    if value == "updated" {
                        break value;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            })
            .await
            .unwrap()
        });

        assert_eq!(actual_value, "updated");
    }

    #[test]
    fn test_not_watch_yaml_file_when_reload_on_change_is_disabled() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        runtime
            .block_on(tokio::fs::write(&file_path, "value: initial"))
            .unwrap();
        let provider = Python::attach(|py| {
            YamlFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
                false,
            )
        });

        Python::attach(|py| provider.load(py)).unwrap();

        assert!(provider.path_watcher.blocking_lock().is_none());
    }
}
