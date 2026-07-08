use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use tokio::fs;

use crate::core::{SerdeParser, SettingsProvider, file_provider};

#[pyclass(str)]
pub struct PythonJsonFileSettingsProvider {
    provider: JsonFileSettingsProvider,
}

#[pymethods]
impl PythonJsonFileSettingsProvider {
    #[new]
    fn new(content_root_path: Option<&str>, path: &str, optional: bool) -> Self {
        Self {
            provider: JsonFileSettingsProvider::new(content_root_path, path, optional),
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

impl fmt::Display for PythonJsonFileSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JsonSettingsProvider")
    }
}

struct JsonFileSettingsProvider {
    data: BTreeMap<String, Option<String>>,
    path: PathBuf,
    optional: bool,
}

impl JsonFileSettingsProvider {
    /// Creates a JSON settings provider from a path.
    ///
    /// The `path` argument can be:
    /// - A file name (for example, `settings.json`).
    /// - A relative path (for example, `config/settings.json`).
    /// - An absolute path (for example, `/tmp/settings.json`).
    ///
    /// File names and relative paths are resolved against `content_root_path` when provided, otherwise against the current working directory.
    fn new(content_root_path: Option<&str>, path: &str, optional: bool) -> Self {
        Self {
            data: BTreeMap::new(),
            path: file_provider::resolve_path(content_root_path, path),
            optional,
        }
    }

    async fn read_json_file(&self) -> PyResult<String> {
        fs::read_to_string(&self.path).await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to read JSON settings file '{}': {}",
                self.path.display(),
                error
            ))
        })
    }
}

impl SettingsProvider for JsonFileSettingsProvider {
    async fn load(&mut self) -> PyResult<()> {
        let file_exists = fs::try_exists(&self.path).await.unwrap_or(false);

        if !file_exists {
            if self.optional {
                return Ok(());
            }

            return Err(PyRuntimeError::new_err(format!(
                "JSON settings file '{}' does not exist",
                self.path.display()
            )));
        }

        let raw_json = self.read_json_file().await?;
        let parsed_json: Value = serde_json::from_str(&raw_json).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Could not parse JSON file '{}': {}",
                self.path.display(),
                error
            ))
        })?;

        let json_object = parsed_json
            .as_object()
            .ok_or_else(|| PyRuntimeError::new_err("JSON root value must be an object"))?;
        let mut parsed_data = SerdeParser::new().parse(json_object)?;
        self.normalize_keys(&mut parsed_data);
        self.data = parsed_data;
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
    use crate::core::SettingsProvider;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::tempdir;

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
        fs::write(&file_path, json.to_string()).unwrap();

        let mut provider = JsonFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);
        provider.load().await.unwrap();

        assert_eq!(provider.data, expected_parsed_json);
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
        fs::write(&file_path, json.to_string()).unwrap();

        let mut provider = JsonFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);
        provider.load().await.unwrap();

        assert_eq!(provider.data, expected_parsed_json);
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
        fs::write(&file_path, json.to_string()).unwrap();

        let mut provider = JsonFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);
        provider.load().await.unwrap();

        assert_eq!(provider.data, expected_parsed_json);
    }
}
