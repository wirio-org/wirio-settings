use pyo3::prelude::*;

use super::PythonSettingsProvider;

/// Source of setting values
#[pyclass(subclass)]
pub struct SettingsSource;

#[pymethods]
impl SettingsSource {
    #[new]
    pub fn new() -> Self {
        Self
    }

    #[allow(clippy::unused_self)]
    fn build(&self) -> PyResult<Py<PythonSettingsProvider>> {
        unimplemented!()
    }
}
