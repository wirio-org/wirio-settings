use arc_swap::ArcSwap;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use tokio::fs;

use crate::core::{
    PathProvider, PythonSettingsProvider, SerdeParser, SettingLookup, SettingsProvider,
};

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct JsonFileSettingsProvider {
    data: ArcSwap<Py<PyDict>>,
    path_provider: PathProvider,
}

#[pymethods]
impl JsonFileSettingsProvider {
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

impl JsonFileSettingsProvider {
    pub fn new(py: Python<'_>, path_provider: PathProvider) -> Self {
        Self {
            data: ArcSwap::from_pointee(PyDict::new(py).unbind()),
            path_provider,
        }
    }

    async fn read_json_file(&self) -> PyResult<String> {
        fs::read_to_string(self.path_provider.path())
            .await
            .map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to read JSON settings file '{}': {}",
                    self.path_provider.path().display(),
                    error
                ))
            })
    }

    fn parse_raw_json(&self, raw_json: &str) -> PyResult<BTreeMap<String, Option<String>>> {
        let parsed_json: Value = serde_json::from_str(raw_json).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Could not parse JSON file '{}': {}",
                self.path_provider.path().display(),
                error
            ))
        })?;

        let json_object = parsed_json
            .as_object()
            .ok_or_else(|| PyRuntimeError::new_err("JSON root value must be an object"))?;

        SerdeParser::new().parse(json_object)
    }
}

impl SettingsProvider for JsonFileSettingsProvider {
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        let data = self.data.load();
        data.clone_ref(py)
    }

    async fn reload(&self) -> PyResult<()> {
        if !self.path_provider.try_is_path_available().await? {
            return Ok(());
        }

        let raw_json = self.read_json_file().await?;
        let mut parsed_data = self.parse_raw_json(&raw_json)?;
        Self::normalize_keys(&mut parsed_data);
        let data = Python::attach(|py| Self::create_data(py, parsed_data))?;
        self.data.store(Arc::new(data));
        Ok(())
    }
}

impl fmt::Display for JsonFileSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}

#[cfg(test)]
mod tests {
    use super::JsonFileSettingsProvider;
    use crate::core::{PathProvider, SettingsProvider};
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use tokio::fs;

    fn assert_data(
        provider: &JsonFileSettingsProvider,
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
    async fn test_parse_scalar_values() {
        let expected_parsed_json = BTreeMap::from([
            (String::from("name"), Some(String::from("wirio"))),
            (String::from("port"), Some(String::from("8080"))),
            (String::from("enabled"), Some(String::from("true"))),
            (String::from("notes"), None),
            (String::from("price"), Some(String::from("19.99"))),
        ]);
        let json = json!({
            "name": "wirio",
            "port": 8080,
            "enabled": true,
            "notes": null,
            "price": 19.99
        });
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.json");
        fs::write(&file_path, json.to_string()).await.unwrap();
        let provider = Python::attach(|py| {
            JsonFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
            )
        });

        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(&provider, &expected_parsed_json);
    }

    #[tokio::test]
    async fn test_parse_nested_objects_and_arrays() {
        let expected_parsed_json = BTreeMap::from([
            (
                String::from("logging.log_level.default"),
                Some(String::from("Information")),
            ),
            (
                String::from("allowed_hosts.0"),
                Some(String::from("localhost")),
            ),
            (
                String::from("allowed_hosts.1"),
                Some(String::from("example.com")),
            ),
        ]);
        let json = json!({
            "Logging": {"LogLevel": {"Default": "Information"}},
            "AllowedHosts": ["localhost", "example.com"]
        });
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.json");
        fs::write(&file_path, json.to_string()).await.unwrap();
        let provider = Python::attach(|py| {
            JsonFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
            )
        });

        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(&provider, &expected_parsed_json);
    }

    #[tokio::test]
    async fn test_set_none_and_empty_for_empty_structures() {
        let expected_parsed_json = BTreeMap::from([
            (String::from("section"), None),
            (String::from("nested_section.section"), None),
            (String::from("items"), Some(String::new())),
            (String::from("nested_items.items"), Some(String::new())),
        ]);
        let json = json!({
            "Section": {},
            "NestedSection": {"Section": {}},
            "Items": [],
            "NestedItems": {"Items": []}
        });
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.json");
        fs::write(&file_path, json.to_string()).await.unwrap();
        let provider = Python::attach(|py| {
            JsonFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, file_path.to_str().unwrap(), false).unwrap(),
            )
        });

        SettingsProvider::reload(&provider).await.unwrap();

        assert_data(&provider, &expected_parsed_json);
    }

    #[tokio::test]
    async fn test_fail_when_checking_file_existence_with_invalid_path() {
        Python::initialize();

        let invalid_file_path = PathBuf::from("\0invalid.json");
        let provider = Python::attach(|py| {
            JsonFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, invalid_file_path.to_str().unwrap(), false).unwrap(),
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();
        let error_message = error.to_string();

        assert!(error_message.contains("RuntimeError: Failed to inspect"));
    }

    #[test]
    fn test_display_returns_type_name() {
        Python::initialize();

        let display = Python::attach(|py| {
            JsonFileSettingsProvider::new(
                py,
                PathProvider::from_file(None, "settings.json", false).unwrap(),
            )
            .to_string()
        });

        assert_eq!(display, "JsonFileSettingsProvider");
    }
}
