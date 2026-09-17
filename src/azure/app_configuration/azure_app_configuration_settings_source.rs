use crate::{
    azure::{
        app_configuration::{
            AzureAppConfigurationSettingsProvider,
            azure_app_configuration_client::AzureAppConfigurationClient,
        },
        identity::PythonAzureCredential,
    },
    core::{PythonSettingsProvider, PythonSettingsSource, SettingsSource},
};
use azure_core::credentials::TokenCredential;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(extends = PythonSettingsSource, frozen)]
pub struct AzureAppConfigurationSettingsSource {
    endpoint: String,
    client: Arc<AzureAppConfigurationClient>,
    credential: Arc<dyn TokenCredential>,
}

#[pymethods]
impl AzureAppConfigurationSettingsSource {
    #[new]
    #[pyo3(signature = (endpoint, credential))]
    pub fn new_python(
        endpoint: String,
        credential: &PythonAzureCredential,
    ) -> PyResult<PyClassInitializer<Self>> {
        let credential = credential.to_token_credential()?;
        let client = Self::create_client(&endpoint, Arc::clone(&credential))?;
        Ok(
            PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
                endpoint,
                client: Arc::new(client),
                credential,
            }),
        )
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl AzureAppConfigurationSettingsSource {
    fn create_client(
        endpoint: &str,
        credential: Arc<dyn TokenCredential>,
    ) -> PyResult<AzureAppConfigurationClient> {
        AzureAppConfigurationClient::new(endpoint, credential).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to create Azure App Configuration client for '{endpoint}': {error}",
            ))
        })
    }
}

impl SettingsSource for AzureAppConfigurationSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                AzureAppConfigurationSettingsProvider::new(
                    py,
                    self.endpoint.clone(),
                    Arc::clone(&self.client),
                    Arc::clone(&self.credential),
                ),
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use crate::_wirio_settings::AzureAppConfigurationSettingsSource;
    use crate::azure::identity::PythonAzureCredential;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;
    use std::sync::Arc;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let python_credential = PythonAzureCredential::ClientSecret {
                tenant_id: String::from("tenant-id"),
                client_id: String::from("client-id"),
                client_secret: String::from("client-secret"),
            };
            let credential = python_credential.to_token_credential().unwrap();
            let source = AzureAppConfigurationSettingsSource {
                endpoint: String::from("https://example.azconfig.io"),
                credential: Arc::clone(&credential),
                client: Arc::new(
                    AzureAppConfigurationSettingsSource::create_client(
                        "https://example.azconfig.io",
                        credential,
                    )
                    .unwrap(),
                ),
            };

            let provider = source.build(py).unwrap();

            assert!(provider.bind(py).is_instance_of::<
                crate::azure::app_configuration::AzureAppConfigurationSettingsProvider,
            >());
        });
    }
}
