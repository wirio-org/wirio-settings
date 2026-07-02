use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use tokio::fs;

use crate::core::{SerdeParser, SettingsProvider, file_provider};

#[pyclass(str)]
pub struct PythonYamlFileSettingsProvider {
    provider: YamlFileSettingsProvider,
}

#[pymethods]
impl PythonYamlFileSettingsProvider {
    #[new]
    fn new(content_root_path: Option<&str>, path: &str, optional: bool) -> Self {
        Self {
            provider: YamlFileSettingsProvider::new(content_root_path, path, optional),
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

impl fmt::Display for PythonYamlFileSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("YamlSettingsProvider")
    }
}

struct YamlFileSettingsProvider {
    pub data: BTreeMap<String, Option<String>>,
    path: PathBuf,
    optional: bool,
}

impl YamlFileSettingsProvider {
    /// Creates a YAML settings provider from a path.
    ///
    /// The `path` argument can be:
    /// - A file name (for example, `settings.yaml`).
    /// - A relative path (for example, `config/settings.yaml`).
    /// - An absolute path (for example, `/tmp/settings.yaml`).
    ///
    /// File names and relative paths are resolved against the current working directory.
    fn new(content_root_path: Option<&str>, path: &str, optional: bool) -> Self {
        Self {
            data: BTreeMap::new(),
            path: file_provider::resolve_path(content_root_path, path),
            optional,
        }
    }

    async fn read_yaml_file(&self) -> PyResult<String> {
        fs::read_to_string(&self.path).await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to read YAML settings file '{}': {}",
                self.path.display(),
                error
            ))
        })
    }
}

impl SettingsProvider for YamlFileSettingsProvider {
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

        let raw_yaml = self.read_yaml_file().await?;

        if raw_yaml.trim().is_empty() {
            self.data = BTreeMap::new();
            return Ok(());
        }

        let parsed_yaml: Value = serde_saphyr::from_str(&raw_yaml).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Could not parse YAML file '{}': {}",
                self.path.display(),
                error
            ))
        })?;
        let yaml_object = parsed_yaml
            .as_object()
            .ok_or_else(|| PyRuntimeError::new_err("Could not parse the YAML file"))?;

        let mut parsed_data = SerdeParser::new().parse(yaml_object)?;
        self.normalize_keys(&mut parsed_data);
        self.data = parsed_data;
        Ok(())
    }
}

impl fmt::Display for YamlFileSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}
