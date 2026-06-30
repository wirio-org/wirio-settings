use crate::core::settings_provider::SettingsProvider;
use pyo3::prelude::*;
use std::collections::BTreeMap;
use std::fmt;

#[pyclass(str)]
pub struct PythonJsonSettingsProvider;

impl fmt::Display for PythonJsonSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        JsonSettingsProvider.fmt(f)
    }
}

#[pymethods]
impl PythonJsonSettingsProvider {
    #[staticmethod]
    pub async fn load() -> PyResult<BTreeMap<String, Option<String>>> {
        JsonSettingsProvider.load().await
    }
}

struct JsonSettingsProvider;

impl JsonSettingsProvider {}

impl SettingsProvider for JsonSettingsProvider {
    async fn load(&self) -> PyResult<BTreeMap<String, Option<String>>> {
        Ok(BTreeMap::new())
    }
}

impl fmt::Display for JsonSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}
