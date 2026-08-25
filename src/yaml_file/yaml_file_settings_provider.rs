use arc_swap::ArcSwap;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer, notify::RecommendedWatcher};
use notify_debouncer_mini::{DebouncedEvent, Debouncer};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use std::{fmt, time::Duration};
use tokio::sync::Mutex;
use tokio::{fs, sync::mpsc::UnboundedSender};
use tokio_util::sync::CancellationToken;

use crate::core::{
    PathProvider, PythonSettingsProvider, SerdeParser, SettingLookup, SettingsProvider,
};

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct YamlFileSettingsProvider {
    data: Arc<ArcSwap<Py<PyDict>>>,
    path_provider: PathProvider,
    reload_on_change: bool,
    watch_file_cancellation_token: Mutex<Option<CancellationToken>>,
    watch_file_debouncer: Mutex<Option<Debouncer<RecommendedWatcher>>>,
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
            watch_file_cancellation_token: Mutex::new(None),
            watch_file_debouncer: Mutex::new(None),
        }
    }

    async fn read_yaml_file(path: &Path) -> PyResult<String> {
        fs::read_to_string(path).await.map_err(|error| {
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
            let cancellation_token = CancellationToken::new();
            self.watch_file_cancellation_token
                .blocking_lock()
                .replace(cancellation_token.clone());
            let (channel_sender, mut channel_receiver) = tokio::sync::mpsc::unbounded_channel();
            let watch_file_debouncer = Self::create_watch_file_debouncer(
                self.path_provider.path(),
                channel_sender,
            )
            .map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to watch YAML settings file '{}': {}",
                        self.path_provider.path().display(),
                        error
                    ))
                })?;
            self.watch_file_debouncer
                .blocking_lock()
                .replace(watch_file_debouncer);
            let runtime = pyo3_async_runtimes::tokio::get_runtime();
            let data = Arc::clone(&self.data);
            let path_provider = self.path_provider.clone();

            runtime.spawn(async move {
                loop {
                    tokio::select! {
                        () = cancellation_token.cancelled() => break,
                        result = channel_receiver.recv() => {
                            let Some(result) = result else {
                                break;
                            };

                            // Ignore errors during file watches
                            if let Ok(events) = result {
                                let are_relevant_file_changes = Self::are_relevant_file_changes(&events);

                                if are_relevant_file_changes {
                                    let _ = Self::reload_settings(&data, &path_provider).await;
                                }
                            }
                        }
                    }
                }
            });

            Ok(())
        })
    }

    fn create_watch_file_debouncer(
        path: &Path,
        channel_sender: UnboundedSender<DebounceEventResult>,
    ) -> notify_debouncer_mini::notify::Result<Debouncer<RecommendedWatcher>> {
        let mut debouncer = new_debouncer(Duration::from_millis(500), move |result| {
            let _ = channel_sender.send(result);
        })?;
        debouncer
            .watcher()
            .watch(path, RecursiveMode::NonRecursive)?;
        Ok(debouncer)
    }

    fn are_relevant_file_changes(events: &[DebouncedEvent]) -> bool {
        let filtered_events: Vec<_> = events
            .iter()
            .filter(|event| !Self::should_ignore_file(&event.path))
            .collect();
        !filtered_events.is_empty()
    }

    /// Exclude files and directories when the name begins with period
    fn should_ignore_file(path: &Path) -> bool {
        path.file_name()
            .is_some_and(|file_name| file_name.as_encoded_bytes().starts_with(b"."))
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

impl Drop for YamlFileSettingsProvider {
    fn drop(&mut self) {
        let watch_file_cancellation_token = self.watch_file_cancellation_token.get_mut().take();

        if let Some(cancellation_token) = watch_file_cancellation_token {
            cancellation_token.cancel();
        }
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
    use tokio::fs;

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
        fs::write(
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
        fs::write(
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
        fs::write(&file_path, "").await.unwrap();

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
        fs::write(
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
            format!("RuntimeError: '{}' does not exist", file_path.display())
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
        fs::write(&file_path, "appName: [wirio").await.unwrap();

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
        fs::write(&file_path, "- wirio\n- config").await.unwrap();

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
        fs::write(&file_path, raw_yaml).await.unwrap();
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

    // #[test]
    // fn test_cancel_watcher_when_dropping_provider() {
    //     Python::initialize();

    //     let cancellation_token = tokio_util::sync::CancellationToken::new();
    //     let provider = Python::attach(|py| {
    //         YamlFileSettingsProvider::new(py, None, "settings.yaml", false, false)
    //     });
    //     provider
    //         .watch_file_cancellation_token
    //         .blocking_lock()
    //         .replace(cancellation_token.clone());

    //     drop(provider);

    //     assert!(cancellation_token.is_cancelled());
    // }
}
