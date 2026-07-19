use crate::core::{PythonSettingsProvider, SettingLookup, SettingsProvider};
use pyo3::prelude::*;
use std::collections::BTreeMap;
use std::fmt;

#[pyclass(extends = PythonSettingsProvider, str)]
pub struct EnvironmentVariablesSettingsProvider {
    data: BTreeMap<String, Option<String>>,
}

#[pymethods]
impl EnvironmentVariablesSettingsProvider {
    #[new]
    pub fn new_python() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(Self::new())
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

impl EnvironmentVariablesSettingsProvider {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    fn get_environment_variables() -> BTreeMap<String, Option<String>> {
        std::env::vars_os()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    Some(value.to_string_lossy().into_owned()),
                )
            })
            .collect()
    }
}

impl SettingsProvider for EnvironmentVariablesSettingsProvider {
    fn data(&self) -> &BTreeMap<String, Option<String>> {
        &self.data
    }

    async fn load(&mut self) -> PyResult<()> {
        let mut environment_variables = Self::get_environment_variables();
        self.normalize_keys(&mut environment_variables);
        self.data = environment_variables;
        Ok(())
    }

    fn section_separator() -> Option<&'static str> {
        Some("__")
    }
}

impl fmt::Display for EnvironmentVariablesSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        core::SettingsProvider, environment_variables::EnvironmentVariablesSettingsProvider,
    };

    #[test]
    fn test_replace_double_underscore_with_dot_in_environment_variable_name() {
        let key = EnvironmentVariablesSettingsProvider::normalize_section_separator(String::from(
            "LOGGING__LOG_LEVEL__DEFAULT",
        ));

        assert_eq!(key, "LOGGING.LOG_LEVEL.DEFAULT");
    }

    #[test]
    fn test_return_same_environment_variable_name_when_no_double_underscore_is_present() {
        let key = EnvironmentVariablesSettingsProvider::normalize_section_separator(String::from(
            "LOGGING",
        ));

        assert_eq!(key, "LOGGING");
    }

    #[test]
    fn test_display_returns_type_name() {
        let expected_display = "EnvironmentVariablesSettingsProvider";

        let display = EnvironmentVariablesSettingsProvider::new().to_string();

        assert_eq!(display, expected_display);
    }
}
