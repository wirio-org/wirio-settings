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
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(extends = PythonSettingsSource, frozen)]
pub struct AzureAppConfigurationSettingsSource {
    endpoint: String,
    client: Arc<AzureAppConfigurationClient>,
}

#[pymethods]
impl AzureAppConfigurationSettingsSource {
    #[new]
    #[pyo3(signature = (endpoint, credential))]
    pub fn new_python(
        endpoint: String,
        credential: &PythonAzureCredential,
    ) -> PyResult<PyClassInitializer<Self>> {
        let client = Self::create_client(&endpoint, credential)?;

        Ok(
            PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
                endpoint,
                client: Arc::new(client),
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
        credential: &PythonAzureCredential,
    ) -> PyResult<AzureAppConfigurationClient> {
        AzureAppConfigurationClient::new(endpoint, credential.to_token_credential()?).map_err(
            |error| {
                PyRuntimeError::new_err(format!(
                    "Failed to create Azure App Configuration client for '{endpoint}': {error}",
                ))
            },
        )
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
                ),
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use super::AzureAppConfigurationSettingsSource;
    use crate::azure::identity::PythonAzureCredential;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;
    use std::sync::Arc;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let credential = PythonAzureCredential::ClientSecret {
                tenant_id: String::from("tenant-id"),
                client_id: String::from("client-id"),
                client_secret: String::from("client-secret"),
            };
            let source = AzureAppConfigurationSettingsSource {
                endpoint: String::from("https://example.azconfig.io"),
                client: Arc::new(
                    AzureAppConfigurationSettingsSource::create_client(
                        "https://example.azconfig.io",
                        &credential,
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
