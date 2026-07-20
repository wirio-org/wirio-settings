use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use tokio::fs;

use crate::core::{
    PythonSettingsProvider, SerdeParser, SettingLookup, SettingsProvider, file_provider,
};

#[pyclass(extends = PythonSettingsProvider, str)]
pub struct YamlFileSettingsProvider {
    data: BTreeMap<String, Option<String>>,
    path: PathBuf,
    optional: bool,
}

#[pymethods]
impl YamlFileSettingsProvider {
    /// Creates a YAML settings provider from a path.
    ///
    /// The `path` argument can be:
    /// - A file name (for example, `settings.yaml`).
    /// - A relative path (for example, `config/settings.yaml`).
    /// - An absolute path (for example, `/tmp/settings.yaml`).
    ///
    /// File names and relative paths are resolved against `content_root_path` when provided, otherwise against the current working directory.
    #[new]
    pub fn new_python(
        content_root_path: Option<&str>,
        path: &str,
        optional: bool,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(Self::new(
            content_root_path,
            path,
            optional,
        ))
    }

    #[getter]
    fn data(&self) -> &BTreeMap<String, Option<String>> {
        SettingsProvider::data(self)
    }

    fn try_get(&self, key: &str) -> SettingLookup {
        SettingsProvider::try_get(self, key)
    }

    pub fn load_sync(&mut self) -> PyResult<()> {
        SettingsProvider::load_sync(self)
    }
}

impl YamlFileSettingsProvider {
    pub fn new(content_root_path: Option<&str>, path: &str, optional: bool) -> Self {
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
    fn data(&self) -> &BTreeMap<String, Option<String>> {
        &self.data
    }

    async fn load(&mut self) -> PyResult<()> {
        let file_exists = fs::try_exists(&self.path).await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to inspect '{}': {}",
                self.path.display(),
                error
            ))
        })?;

        if !file_exists {
            if self.optional {
                return Ok(());
            }

            return Err(PyRuntimeError::new_err(format!(
                "YAML settings file '{}' does not exist",
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

        if parsed_yaml.is_null() {
            self.data = BTreeMap::new();
            return Ok(());
        }

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

#[cfg(test)]
mod tests {
    use super::YamlFileSettingsProvider;
    use crate::core::SettingsProvider;
    use pyo3::Python;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use tokio::fs;

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

        let mut provider = YamlFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);
        SettingsProvider::load(&mut provider).await.unwrap();

        assert_eq!(
            provider.data,
            BTreeMap::from([
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
                    Some(String::from("warning"))
                ),
                (String::from("logging.log_level.notes"), None),
            ])
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

        let mut provider = YamlFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);
        SettingsProvider::load(&mut provider).await.unwrap();

        assert_eq!(
            provider.data,
            BTreeMap::from([
                (String::from("app_name"), Some(String::from("wirio"))),
                (String::from("port"), Some(String::from("8080"))),
            ])
        );
    }

    #[tokio::test]
    async fn test_return_empty_data_when_yaml_file_is_empty() {
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        fs::write(&file_path, "").await.unwrap();

        let mut provider = YamlFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);
        SettingsProvider::load(&mut provider).await.unwrap();

        assert_eq!(provider.data, BTreeMap::new());
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

        let mut provider = YamlFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);
        SettingsProvider::load(&mut provider).await.unwrap();

        assert_eq!(provider.data, BTreeMap::new());
    }

    #[tokio::test]
    async fn test_return_empty_data_when_optional_file_is_missing() {
        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("missing.yaml");

        let mut provider = YamlFileSettingsProvider::new(None, file_path.to_str().unwrap(), true);
        SettingsProvider::load(&mut provider).await.unwrap();

        assert_eq!(provider.data, BTreeMap::new());
    }

    #[tokio::test]
    async fn test_fail_when_required_file_is_missing() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("missing.yaml");

        let mut provider = YamlFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);

        let error = SettingsProvider::load(&mut provider).await.unwrap_err();
        let error_message = error.to_string();

        assert_eq!(
            error_message,
            format!(
                "RuntimeError: YAML settings file '{}' does not exist",
                file_path.display()
            )
        );
    }

    #[tokio::test]
    async fn test_fail_when_path_points_to_directory() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let mut provider = YamlFileSettingsProvider::new(
            None,
            temporary_directory.path().to_str().unwrap(),
            false,
        );

        let error = SettingsProvider::load(&mut provider).await.unwrap_err();
        let error_message = error.to_string();

        assert!(error_message.contains("Failed to read YAML settings file"));
    }

    #[tokio::test]
    async fn test_fail_when_yaml_has_invalid_syntax() {
        Python::initialize();

        let temporary_directory = tempdir().unwrap();
        let file_path = temporary_directory.path().join("settings.yaml");
        fs::write(&file_path, "appName: [wirio").await.unwrap();

        let mut provider = YamlFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);

        let error = SettingsProvider::load(&mut provider).await.unwrap_err();
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

        let mut provider = YamlFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);

        let error = SettingsProvider::load(&mut provider).await.unwrap_err();
        let error_message = error.to_string();

        assert!(error_message.contains("Could not parse the YAML file"));
    }

    #[test]
    fn test_display_returns_type_name() {
        let display = YamlFileSettingsProvider::new(None, "settings.yaml", false).to_string();

        assert_eq!(display, "YamlFileSettingsProvider");
    }

    #[tokio::test]
    async fn test_fail_when_checking_file_existence_with_invalid_path() {
        Python::initialize();

        let invalid_file_path = PathBuf::from("\0invalid.yaml");
        let mut provider =
            YamlFileSettingsProvider::new(None, invalid_file_path.to_str().unwrap(), false);

        let error = SettingsProvider::load(&mut provider).await.unwrap_err();
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
        let mut provider = YamlFileSettingsProvider::new(None, file_path.to_str().unwrap(), false);

        SettingsProvider::load(&mut provider).await.unwrap();

        assert_eq!(provider.data, expected_parsed_yaml);
    }
}
