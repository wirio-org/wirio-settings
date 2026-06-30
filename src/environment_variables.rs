use crate::core::settings_provider::SettingsProvider;
use pyo3::prelude::*;
use std::collections::BTreeMap;
use std::fmt;

#[pyclass(str)]
pub struct PythonEnvironmentVariablesSettingsProvider;

#[pymethods]
impl PythonEnvironmentVariablesSettingsProvider {
    #[staticmethod]
    pub async fn load() -> PyResult<BTreeMap<String, Option<String>>> {
        EnvironmentVariablesSettingsProvider.load().await
    }
}

impl fmt::Display for PythonEnvironmentVariablesSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        EnvironmentVariablesSettingsProvider.fmt(f)
    }
}

struct EnvironmentVariablesSettingsProvider;

impl EnvironmentVariablesSettingsProvider {
    fn get_environment_variables(&self) -> PyResult<BTreeMap<String, Option<String>>> {
        Python::attach(|python| -> PyResult<BTreeMap<String, Option<String>>> {
            let environ = python.import("os")?.getattr("environ")?;
            let dict_type = python.import("builtins")?.getattr("dict")?;
            let environment_data: BTreeMap<String, String> =
                dict_type.call1((environ,))?.extract()?;
            let environment_data_with_optional_values = environment_data
                .into_iter()
                .map(|(key, value)| (key, Some(value)))
                .collect();
            Ok(environment_data_with_optional_values)
        })
    }
}

impl SettingsProvider for EnvironmentVariablesSettingsProvider {
    async fn load(&self) -> PyResult<BTreeMap<String, Option<String>>> {
        let mut environment_variables = self.get_environment_variables()?;
        self.normalize_keys(&mut environment_variables);
        Ok(environment_variables)
    }

    fn section_separator(&self) -> Option<&str> {
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
        core::settings_provider::SettingsProvider,
        environment_variables::EnvironmentVariablesSettingsProvider,
    };
    use pyo3::{prelude::*, types::PyDict};
    use std::collections::BTreeMap;

    #[test]
    fn test_replace_double_underscore_with_dot_in_environment_variable_name() {
        let key = EnvironmentVariablesSettingsProvider
            .normalize_section_separator(String::from("LOGGING__LOG_LEVEL__DEFAULT"));

        assert_eq!(key, "LOGGING.LOG_LEVEL.DEFAULT");
    }

    #[test]
    fn test_return_same_environment_variable_name_when_no_double_underscore_is_present() {
        let key = EnvironmentVariablesSettingsProvider
            .normalize_section_separator(String::from("LOGGING"));

        assert_eq!(key, "LOGGING");
    }

    #[tokio::test]
    #[ignore]
    async fn test_load_environment_variables() {
        Python::initialize();

        let expected_environment_variables = BTreeMap::from([(
            String::from("logging.log_level.default"),
            Some(String::from("WARNING")),
        )]);

        let original_environ = Python::attach(|python| -> PyResult<Py<PyAny>> {
            let os_module = python.import("os")?;
            let dict_type = python.import("builtins")?.getattr("dict")?;
            let original_environ = dict_type.call1((os_module.getattr("environ")?,))?;
            let environ_mock = PyDict::new(python);
            environ_mock.set_item("LOGGING__LOG_LEVEL__DEFAULT", "WARNING")?;
            os_module.setattr("environ", environ_mock)?;
            Ok(original_environ.unbind())
        })
        .unwrap();

        let data_result = EnvironmentVariablesSettingsProvider.load().await;

        Python::attach(|python| -> PyResult<()> {
            let os_module = python.import("os")?;
            os_module.setattr("environ", original_environ.bind(python))?;
            Ok(())
        })
        .unwrap();

        let data = data_result.unwrap();

        assert_eq!(data, expected_environment_variables);
    }

    #[test]
    fn test_display_returns_type_name() {
        let expected_display = "EnvironmentVariablesSettingsProvider";

        let display = EnvironmentVariablesSettingsProvider.to_string();

        assert_eq!(display, expected_display);
    }
}
