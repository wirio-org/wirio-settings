use pyo3::prelude::*;

use super::PythonSettingsProvider;

/// Source of setting values
#[pyclass(subclass, name = "SettingsSource")]
pub struct PythonSettingsSource;

#[pymethods]
impl PythonSettingsSource {
    #[new]
    pub fn new() -> Self {
        Self
    }

    #[allow(clippy::unused_self)]
    fn build(&self, _py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        unimplemented!()
    }
}

pub trait SettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>>;
}
