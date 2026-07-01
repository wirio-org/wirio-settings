use super::json_settings_parser::JsonSettingsParser;
use crate::core::settings_provider::SettingsProvider;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use tokio::fs;

#[pyclass(str)]
pub struct PythonJsonSettingsProvider {
    provider: JsonSettingsProvider,
}

#[pymethods]
impl PythonJsonSettingsProvider {
    #[new]
    fn new(path: &str, optional: bool) -> Self {
        Self {
            provider: JsonSettingsProvider::new(path, optional),
        }
    }

    #[getter]
    fn data(&self) -> &BTreeMap<String, Option<String>> {
        &self.provider.data
    }

    pub async fn load(&mut self) -> PyResult<()> {
        self.provider.load().await
    }
}

impl fmt::Display for PythonJsonSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JsonSettingsProvider")
    }
}

struct JsonSettingsProvider {
    pub data: BTreeMap<String, Option<String>>,
    path: PathBuf,
    optional: bool,
}

impl JsonSettingsProvider {
    /// Creates a JSON settings provider from a path.
    ///
    /// The `path` argument can be:
    /// - A file name (for example, `settings.json`).
    /// - A relative path (for example, `config/settings.json`).
    /// - An absolute path (for example, `/tmp/settings.json`).
    ///
    /// File names and relative paths are resolved against the current working directory.
    fn new(path: &str, optional: bool) -> Self {
        let path = PathBuf::from(path);

        let calculated_path = if path.has_root() {
            path
        } else {
            let current_directory = std::env::current_dir().unwrap();
            current_directory.join(path)
        };

        Self {
            data: BTreeMap::new(),
            path: calculated_path,
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

impl SettingsProvider for JsonSettingsProvider {
    async fn load(&mut self) -> PyResult<()> {
        if !self.path.exists() {
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
            .ok_or_else(|| PyRuntimeError::new_err("Could not parse the JSON file"))?;
        let mut parsed_data = JsonSettingsParser::new().parse(json_object)?;
        self.normalize_keys(&mut parsed_data);
        self.data = parsed_data;
        Ok(())
    }
}

impl fmt::Display for JsonSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}
