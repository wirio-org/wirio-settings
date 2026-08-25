use crate::{
    core::{PathProvider, PythonSettingsProvider, PythonSettingsSource, SettingsSource},
    json_file::JsonFileSettingsProvider,
};
use pyo3::prelude::*;

#[pyclass(extends = PythonSettingsSource, frozen)]
pub struct JsonFileSettingsSource {
    path_provider: PathProvider,
    reload_on_change: bool,
}

#[pymethods]
impl JsonFileSettingsSource {
    #[new]
    pub fn new_python(
        content_root_path: Option<&str>,
        path: &str,
        optional: bool,
        reload_on_change: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
                path_provider: PathProvider::from_file(content_root_path, path, optional)?,
                reload_on_change,
            }),
        )
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl SettingsSource for JsonFileSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                JsonFileSettingsProvider::new(
                    py,
                    self.path_provider.clone(),
                    self.reload_on_change,
                ),
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use crate::core::PathProvider;

    use super::JsonFileSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = JsonFileSettingsSource {
                path_provider: PathProvider::from_file(None, "settings.json", false).unwrap(),
                reload_on_change: false,
            };

            let provider = source.build(py).unwrap();

            assert!(
                provider
                    .bind(py)
                    .is_instance_of::<crate::json_file::JsonFileSettingsProvider>()
            );
        });
    }
}
