use crate::{
    core::{PythonSettingsProvider, PythonSettingsSource, SettingsSource},
    gcp_secret_manager::GcpSecretManagerSettingsProvider,
};
use pyo3::prelude::*;

#[pyclass(extends = PythonSettingsSource)]
pub struct GcpSecretManagerSettingsSource {
    project_id: String,
    credentials_json: Option<String>,
}

#[pymethods]
impl GcpSecretManagerSettingsSource {
    #[new]
    #[pyo3(signature = (project_id, credentials_json=None))]
    pub fn new_python(
        project_id: String,
        credentials_json: Option<String>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
            project_id,
            credentials_json,
        })
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl SettingsSource for GcpSecretManagerSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                GcpSecretManagerSettingsProvider::new(
                    self.project_id.clone(),
                    self.credentials_json.clone(),
                ),
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use super::GcpSecretManagerSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = GcpSecretManagerSettingsSource {
                project_id: String::from("project"),
                credentials_json: None,
            };

            let provider = source.build(py).unwrap();

            assert!(
                provider
                    .bind(py)
                    .is_instance_of::<crate::gcp_secret_manager::GcpSecretManagerSettingsProvider>(
                    )
            );
        });
    }
}
