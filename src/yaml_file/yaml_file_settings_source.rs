use crate::{
    core::{PathProvider, PythonSettingsProvider, PythonSettingsSource, SettingsSource},
    yaml_file::YamlFileSettingsProvider,
};
use pyo3::prelude::*;

#[pyclass(extends = PythonSettingsSource, frozen)]
pub struct YamlFileSettingsSource {
    path_provider: PathProvider,
    reload_on_change: bool,
}

#[pymethods]
impl YamlFileSettingsSource {
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

impl SettingsSource for YamlFileSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                YamlFileSettingsProvider::new(
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

    use super::YamlFileSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = YamlFileSettingsSource {
                path_provider: PathProvider::from_file(None, "settings.yaml", false).unwrap(),
                reload_on_change: false,
            };

            let provider = source.build(py).unwrap();

            assert!(
                provider
                    .bind(py)
                    .is_instance_of::<crate::yaml_file::YamlFileSettingsProvider>()
            );
        });
    }
}
