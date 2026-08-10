use pyo3::prelude::*;

use super::PythonSettingsProvider;

/// Source of setting values
#[pyclass(name = "SettingsSource", subclass)]
pub struct PythonSettingsSource;

#[pymethods]
impl PythonSettingsSource {
    #[new]
    pub fn new() -> Self {
        Self
    }

    #[allow(clippy::unused_self)]
    #[allow(unused_variables)]
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        unimplemented!()
    }
}

pub trait SettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>>;
}
