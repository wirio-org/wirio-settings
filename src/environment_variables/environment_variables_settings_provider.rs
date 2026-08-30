use crate::core::{ModelRegistry, PythonSettingsProvider, SettingLookup, SettingsProvider};
use arc_swap::ArcSwap;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::OnceCell;

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct EnvironmentVariablesSettingsProvider {
    data: ArcSwap<Py<PyDict>>,
    model_registry: OnceCell<Py<ModelRegistry>>,
}

#[pymethods]
impl EnvironmentVariablesSettingsProvider {
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

    fn set_model_registry(&self, model_registry: PyRef<'_, ModelRegistry>) -> PyResult<()> {
        SettingsProvider::set_model_registry(self, model_registry)
    }
}

impl EnvironmentVariablesSettingsProvider {
    pub fn new(py: Python<'_>) -> Self {
        Self {
            data: ArcSwap::from_pointee(PyDict::new(py).unbind()),
            model_registry: OnceCell::new(),
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
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        let data = self.data.load();
        data.clone_ref(py)
    }

    async fn reload(&self) -> PyResult<()> {
        let mut environment_variables = Self::get_environment_variables();
        Self::normalize_keys(&mut environment_variables);
        let data = Python::attach(|py| Self::create_data(py, environment_variables))?;
        self.data.store(Arc::new(data));
        Python::attach(|py| Self::on_reload(py, self.model_registry()));
        Ok(())
    }

    fn section_separator() -> Option<&'static str> {
        Some("__")
    }

    fn model_registry(&self) -> &OnceCell<Py<ModelRegistry>> {
        &self.model_registry
    }
}

impl fmt::Display for EnvironmentVariablesSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}

#[cfg(test)]
mod tests {
    use super::EnvironmentVariablesSettingsProvider;
    use crate::core::{ModelRegistry, SettingsProvider};
    use pyo3::{
        Py, Python,
        types::{PyAnyMethods, PyModule, PyWeakrefReference},
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

        Python::initialize();

        let display =
            Python::attach(|py| EnvironmentVariablesSettingsProvider::new(py).to_string());

        assert_eq!(display, expected_display);
    }

    #[test]
    fn test_set_model_registry() {
        Python::initialize();

        Python::attach(|py| {
            let module = PyModule::from_code(py, c"def callback():\n    pass\n", c"", c"").unwrap();
            let callback = module.getattr("callback").unwrap();
            let callback_reference = PyWeakrefReference::new(&callback).unwrap().unbind();
            let model_registry = Py::new(py, ModelRegistry::new(py, callback_reference)).unwrap();
            let provider = EnvironmentVariablesSettingsProvider::new(py);

            provider
                .set_model_registry(model_registry.bind(py).borrow())
                .unwrap();

            assert!(
                provider
                    .model_registry()
                    .get()
                    .unwrap()
                    .bind(py)
                    .is(model_registry.bind(py))
            );
        });
    }
}
