use crate::core::{settings_path::SettingsPath, settings_provider::SettingsProvider};
use pyo3::prelude::*;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::{Display, Formatter};

#[pyclass(str)]
pub struct InternalEnvironmentVariablesSettingsProvider;

#[pymethods]
impl InternalEnvironmentVariablesSettingsProvider {
    #[staticmethod]
    pub async fn load() -> PyResult<BTreeMap<String, Option<String>>> {
        let mut data: BTreeMap<String, Option<String>> =
            Python::attach(|python| -> PyResult<BTreeMap<String, Option<String>>> {
                let environ = python.import("os")?.getattr("environ")?;
                let dict_type = python.import("builtins")?.getattr("dict")?;
                let environment_data: BTreeMap<String, String> =
                    dict_type.call1((environ,))?.extract()?;
                let mut loaded_data: BTreeMap<String, Option<String>> = BTreeMap::new();

                for (environment_variable_key, environment_variable_value) in environment_data {
                    let normalized_environment_variable_key =
                        InternalEnvironmentVariablesSettingsProvider::normalize_key(
                            &environment_variable_key,
                        );
                    loaded_data.insert(
                        normalized_environment_variable_key,
                        Some(environment_variable_value),
                    );
                }

                Ok(loaded_data)
            })?;

        SettingsProvider::normalize_loaded_data(&mut data);
        Ok(data)
    }

    #[staticmethod]
    fn normalize_key(key: &str) -> String {
        key.replace("__", SettingsPath::KEY_DELIMITER)
    }
}

impl Display for InternalEnvironmentVariablesSettingsProvider {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let full_name = std::any::type_name::<Self>();
        let short_name = full_name.split("::").last().unwrap_or(full_name);
        write!(f, "{}", short_name)
    }
}

#[cfg(test)]
mod tests {
    use super::InternalEnvironmentVariablesSettingsProvider;
    use pyo3::{prelude::*, types::PyDict};
    use std::collections::BTreeMap;

    #[test]
    fn test_replace_double_underscore_with_dot_in_environment_variable_name() {
        let key = InternalEnvironmentVariablesSettingsProvider::normalize_key(
            "LOGGING__LOG_LEVEL__DEFAULT",
        );

        assert_eq!(key, "LOGGING.LOG_LEVEL.DEFAULT");
    }

    #[test]
    fn test_return_same_environment_variable_name_when_no_double_underscore_is_present() {
        let key = InternalEnvironmentVariablesSettingsProvider::normalize_key("LOGGING");

        assert_eq!(key, "LOGGING");
    }

    #[tokio::test]
    async fn test_load_environment_variables() {
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

        let data_result = InternalEnvironmentVariablesSettingsProvider::load().await;

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
        let expected_display = "InternalEnvironmentVariablesSettingsProvider";

        let display = InternalEnvironmentVariablesSettingsProvider.to_string();

        assert_eq!(display, expected_display);
    }
}
