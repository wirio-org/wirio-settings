use super::remove_user_agent::RemoveUserAgent;
use crate::{
    azure::{identity::PythonAzureCredential, key_vault::AzureKeyVaultSettingsProvider},
    core::{PythonSettingsProvider, PythonSettingsSource, SettingsSource},
};
use azure_security_keyvault_secrets::{SecretClient, SecretClientOptions};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::sync::Arc;
use std::time::Duration;

#[pyclass(extends = PythonSettingsSource, frozen)]
pub struct AzureKeyVaultSettingsSource {
    url: String,
    secret_client: Arc<SecretClient>,
    reload_interval: Option<Duration>,
}

#[pymethods]
impl AzureKeyVaultSettingsSource {
    #[new]
    #[pyo3(signature = (url, credential, reload_interval=None))]
    pub fn new_python(
        url: String,
        credential: &PythonAzureCredential,
        reload_interval: Option<Duration>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let secret_client = Self::create_secret_client(&url, credential)?;

        Ok(
            PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
                url,
                secret_client: Arc::new(secret_client),
                reload_interval,
            }),
        )
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl AzureKeyVaultSettingsSource {
    fn create_secret_client(
        url: &str,
        credential: &PythonAzureCredential,
    ) -> PyResult<SecretClient> {
        let credential = credential.to_token_credential()?;
        let mut client_options = SecretClientOptions::default();
        client_options
            .client_options
            .per_call_policies
            .push(Arc::new(RemoveUserAgent));

        SecretClient::new(url, credential, Some(client_options)).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to create Azure Key Vault client for '{url}': {error}",
            ))
        })
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
                    Arc::clone(&self.secret_client),
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
    use crate::azure::identity::PythonAzureCredential;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;
    use std::sync::Arc;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let credential = &PythonAzureCredential::ClientSecret {
                tenant_id: String::from("tenant-id"),
                client_id: String::from("client-id"),
                client_secret: String::from("client-secret"),
            };
            let source = AzureKeyVaultSettingsSource {
                url: String::from("https://example.vault.azure.net"),
                secret_client: Arc::new(
                    AzureKeyVaultSettingsSource::create_secret_client(
                        "https://example.vault.azure.net",
                        credential,
                    )
                    .unwrap(),
                ),
                reload_interval: None,
            };

            let provider = source.build(py).unwrap();

            assert!(
                provider
                    .bind(py)
                    .is_instance_of::<crate::azure::key_vault::AzureKeyVaultSettingsProvider>()
            );
        });
    }
}
