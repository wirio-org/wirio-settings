use crate::{
    core::{PythonSettingsProvider, SettingsSource},
    environment_variables::EnvironmentVariablesSettingsProvider,
};
use pyo3::prelude::*;

#[pyclass(extends = SettingsSource)]
pub struct EnvironmentVariablesSettingsSource;

#[pymethods]
impl EnvironmentVariablesSettingsSource {
    #[new]
    pub fn new_python() -> PyClassInitializer<Self> {
        PyClassInitializer::from(SettingsSource::new()).add_subclass(Self)
    }

    #[allow(clippy::unused_self)]
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new())
                .add_subclass(EnvironmentVariablesSettingsProvider::new()),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use super::EnvironmentVariablesSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = EnvironmentVariablesSettingsSource;

            let provider = source.build(py).unwrap();

            assert!(provider
                .bind(py)
                .is_instance_of::<crate::environment_variables::EnvironmentVariablesSettingsProvider>());
        });
    }
}
