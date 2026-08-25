use crate::{
    core::{PathProvider, PythonSettingsProvider, PythonSettingsSource, SettingsSource},
    key_per_file::KeyPerFileSettingsProvider,
};
use pyo3::prelude::*;

#[pyclass(extends = PythonSettingsSource, frozen)]
pub struct KeyPerFileSettingsSource {
    path_provider: PathProvider,
}

#[pymethods]
impl KeyPerFileSettingsSource {
    #[new]
    pub fn new_python(directory_path: &str, optional: bool) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
                path_provider: PathProvider::from_directory(directory_path, optional)?,
            }),
        )
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl SettingsSource for KeyPerFileSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                KeyPerFileSettingsProvider::new(py, self.path_provider.clone()),
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use crate::core::PathProvider;

    use super::KeyPerFileSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = KeyPerFileSettingsSource {
                path_provider: PathProvider::from_directory(
                    std::env::current_dir().unwrap().to_str().unwrap(),
                    false,
                )
                .unwrap(),
            };

            let provider = source.build(py).unwrap();

            assert!(
                provider
                    .bind(py)
                    .is_instance_of::<crate::key_per_file::KeyPerFileSettingsProvider>()
            );
        });
    }
}
