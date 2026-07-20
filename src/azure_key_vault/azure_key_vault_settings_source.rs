use crate::{
    azure_key_vault::AzureKeyVaultSettingsProvider,
    core::{PythonSettingsProvider, SettingsSource},
};
use pyo3::prelude::*;

#[pyclass(extends = SettingsSource)]
pub struct AzureKeyVaultSettingsSource {
    url: String,
    client_id: Option<String>,
    client_secret: Option<String>,
    tenant_id: Option<String>,
}

#[pymethods]
impl AzureKeyVaultSettingsSource {
    #[new]
    #[pyo3(signature = (url, client_id=None, client_secret=None, tenant_id=None))]
    pub fn new_python(
        url: String,
        client_id: Option<String>,
        client_secret: Option<String>,
        tenant_id: Option<String>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(SettingsSource::new()).add_subclass(Self {
            url,
            client_id,
            client_secret,
            tenant_id,
        })
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                AzureKeyVaultSettingsProvider::new(
                    self.url.clone(),
                    self.client_id.clone(),
                    self.client_secret.clone(),
                    self.tenant_id.clone(),
                ),
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
