use crate::{
    core::{PythonSettingsProvider, SettingsSource},
    json_file::JsonFileSettingsProvider,
};
use pyo3::prelude::*;

#[pyclass(extends = SettingsSource)]
pub struct JsonFileSettingsSource {
    content_root_path: Option<String>,
    path: String,
    optional: bool,
}

#[pymethods]
impl JsonFileSettingsSource {
    #[new]
    pub fn new_python(
        content_root_path: Option<String>,
        path: String,
        optional: bool,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(SettingsSource::new()).add_subclass(Self {
            content_root_path,
            path,
            optional,
        })
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                JsonFileSettingsProvider::new(
                    self.content_root_path.as_deref(),
                    &self.path,
                    self.optional,
                ),
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use super::JsonFileSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = JsonFileSettingsSource {
                content_root_path: None,
                path: String::from("settings.json"),
                optional: false,
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
