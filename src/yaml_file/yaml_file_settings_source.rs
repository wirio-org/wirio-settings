use crate::{
    core::{PythonSettingsProvider, SettingsSource},
    yaml_file::YamlFileSettingsProvider,
};
use pyo3::prelude::*;

#[pyclass(extends = SettingsSource)]
pub struct YamlFileSettingsSource {
    content_root_path: Option<String>,
    path: String,
    optional: bool,
}

#[pymethods]
impl YamlFileSettingsSource {
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
                YamlFileSettingsProvider::new(
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
    use super::YamlFileSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = YamlFileSettingsSource {
                content_root_path: None,
                path: String::from("settings.yaml"),
                optional: false,
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
