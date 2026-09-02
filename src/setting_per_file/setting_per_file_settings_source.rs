use crate::{
    core::{PathProvider, PythonSettingsProvider, PythonSettingsSource, SettingsSource},
    setting_per_file::SettingPerFileSettingsProvider,
};
use pyo3::prelude::*;

#[pyclass(extends = PythonSettingsSource, frozen)]
pub struct SettingPerFileSettingsSource {
    path_provider: PathProvider,
    reload_on_change: bool,
}

#[pymethods]
impl SettingPerFileSettingsSource {
    #[new]
    pub fn new_python(
        directory_path: &str,
        optional: bool,
        reload_on_change: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
                path_provider: PathProvider::from_directory(directory_path, optional)?,
                reload_on_change,
            }),
        )
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl SettingsSource for SettingPerFileSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                SettingPerFileSettingsProvider::new(
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

    use super::SettingPerFileSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = SettingPerFileSettingsSource {
                path_provider: PathProvider::from_directory(
                    std::env::current_dir().unwrap().to_str().unwrap(),
                    false,
                )
                .unwrap(),
                reload_on_change: false,
            };

            let provider = source.build(py).unwrap();

            assert!(
                provider
                    .bind(py)
                    .is_instance_of::<crate::setting_per_file::SettingPerFileSettingsProvider>()
            );
        });
    }
}
