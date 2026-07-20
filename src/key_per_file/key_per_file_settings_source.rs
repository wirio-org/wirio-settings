use crate::{
    core::{PythonSettingsProvider, SettingsSource},
    key_per_file::KeyPerFileSettingsProvider,
};
use pyo3::prelude::*;

#[pyclass(extends = SettingsSource)]
pub struct KeyPerFileSettingsSource {
    directory_path: String,
    optional: bool,
}

#[pymethods]
impl KeyPerFileSettingsSource {
    #[new]
    pub fn new_python(directory_path: String, optional: bool) -> PyClassInitializer<Self> {
        PyClassInitializer::from(SettingsSource::new()).add_subclass(Self {
            directory_path,
            optional,
        })
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                KeyPerFileSettingsProvider::new(&self.directory_path, self.optional),
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use super::KeyPerFileSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = KeyPerFileSettingsSource {
                directory_path: String::from("settings"),
                optional: false,
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
