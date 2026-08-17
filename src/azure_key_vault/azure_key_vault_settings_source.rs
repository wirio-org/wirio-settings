use crate::{
    azure_key_vault::AzureKeyVaultSettingsProvider,
    core::{PythonSettingsProvider, PythonSettingsSource, SettingsSource},
};
use pyo3::prelude::*;
use std::time::Duration;

#[pyclass(extends = PythonSettingsSource, frozen)]
pub struct AzureKeyVaultSettingsSource {
    url: String,
    tenant_id: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    reload_interval: Option<Duration>,
}

#[pymethods]
impl AzureKeyVaultSettingsSource {
    #[new]
    #[pyo3(signature = (url, tenant_id=None, client_id=None, client_secret=None, reload_interval=None))]
    pub fn new_python(
        url: String,
        tenant_id: Option<String>,
        client_id: Option<String>,
        client_secret: Option<String>,
        reload_interval: Option<Duration>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
            url,
            tenant_id,
            client_id,
            client_secret,
            reload_interval,
        })
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl SettingsSource for AzureKeyVaultSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                AzureKeyVaultSettingsProvider::new(
                    py,
                    self.url.clone(),
                    self.tenant_id.clone(),
                    self.client_id.clone(),
                    self.client_secret.clone(),
                    self.reload_interval,
                )?,
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use super::AzureKeyVaultSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = AzureKeyVaultSettingsSource {
                url: String::from("https://example.vault.azure.net"),
                client_id: None,
                client_secret: None,
                tenant_id: None,
                reload_interval: None,
            };

            let provider = source.build(py).unwrap();

            assert!(
                provider
                    .bind(py)
                    .is_instance_of::<crate::azure_key_vault::AzureKeyVaultSettingsProvider>()
            );
        });
    }
}
