use arc_swap::ArcSwap;
use azure_identity::ClientSecretCredential;
use azure_security_keyvault_secrets::SecretClientOptions;
use azure_security_keyvault_secrets::{ResourceExt, SecretClient, models::SecretProperties};
use futures::TryStreamExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::azure_key_vault::default_azure_credential::DefaultAzureCredential;
use crate::azure_key_vault::remove_user_agent::RemoveUserAgent;
use crate::core::{PythonSettingsProvider, SettingLookup, SettingsProvider};

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct AzureKeyVaultSettingsProvider {
    data: ArcSwap<Py<PyDict>>,
    // internal_data: RwLock<BTreeMap<String, Option<String>>>,
    url: String,
    client_id: Option<String>,
    client_secret: Option<String>,
    tenant_id: Option<String>,
    #[allow(dead_code)]
    reload_interval: Option<Duration>,
}

#[pymethods]
impl AzureKeyVaultSettingsProvider {
    #[new]
    #[pyo3(signature = (url, client_id=None, client_secret=None, tenant_id=None, reload_interval=None))]
    pub fn new_python(
        py: Python<'_>,
        url: String,
        client_id: Option<String>,
        client_secret: Option<String>,
        tenant_id: Option<String>,
        reload_interval: Option<Duration>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(Self::new(
            py,
            url,
            client_id,
            client_secret,
            tenant_id,
            reload_interval,
        ))
    }

    #[pyo3(signature = () -> "dict[str, str | None]")]
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        SettingsProvider::data(self, py)
    }

    fn try_get(&self, py: Python<'_>, key: &str) -> PyResult<SettingLookup> {
        SettingsProvider::try_get(self, py, key)
    }

    pub fn load_sync(&self) -> PyResult<()> {
        SettingsProvider::load_sync(self)
    }
}

impl AzureKeyVaultSettingsProvider {
    pub fn new(
        py: Python<'_>,
        url: String,
        client_id: Option<String>,
        client_secret: Option<String>,
        tenant_id: Option<String>,
        reload_interval: Option<Duration>,
    ) -> Self {
        Self {
            data: ArcSwap::from_pointee(PyDict::new(py).unbind()),
            url,
            client_id,
            client_secret,
            tenant_id,
            reload_interval,
        }
    }

    fn create_secret_client(&self) -> PyResult<SecretClient> {
        self.validate_explicit_credentials()?;

        if self.has_explicit_credentials() {
            let tenant_id = self.tenant_id.clone().ok_or_else(|| {
                PyRuntimeError::new_err("Missing 'tenant_id' for explicit Azure credentials")
            })?;
            let client_id = self.client_id.clone().ok_or_else(|| {
                PyRuntimeError::new_err("Missing 'client_id' for explicit Azure credentials")
            })?;
            let client_secret = self.client_secret.clone().ok_or_else(|| {
                PyRuntimeError::new_err("Missing 'client_secret' for explicit Azure credentials")
            })?;
            let credential = ClientSecretCredential::new(
                &tenant_id,
                client_id,
                client_secret.into(),
                None,
            )
            .map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to create explicit Azure credential for Azure Key Vault: {error}"
                ))
            })?;

            let remove_user_agent = Arc::new(RemoveUserAgent);

            // Construct client options with our policy, that runs after the built-in per-call UserAgentPolicy
            let mut secret_client_options = SecretClientOptions::default();
            secret_client_options
                .client_options
                .per_call_policies
                .push(remove_user_agent);

            return SecretClient::new(&self.url, credential, None).map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to create Azure Key Vault client for '{}': {error}",
                    self.url
                ))
            });
        }

        let credential = DefaultAzureCredential::new();
        SecretClient::new(&self.url, credential, None).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to create Azure Key Vault client for '{}': {error}",
                self.url
            ))
        })
    }

    fn validate_explicit_credentials(&self) -> PyResult<()> {
        if self.has_explicit_credentials()
            && (self.tenant_id.is_none()
                || self.client_id.is_none()
                || self.client_secret.is_none())
        {
            return Err(PyRuntimeError::new_err(
                "'tenant_id', 'client_id', and 'client_secret' must all be provided when using explicit Azure credentials",
            ));
        }

        Ok(())
    }

    fn has_explicit_credentials(&self) -> bool {
        self.tenant_id.is_some() || self.client_id.is_some() || self.client_secret.is_some()
    }

    fn is_secret_enabled(secret_properties: &SecretProperties) -> bool {
        secret_properties
            .attributes
            .as_ref()
            .and_then(|attributes| attributes.enabled)
            .unwrap_or(false)
    }

    fn extract_secret_name(secret_properties: &SecretProperties) -> PyResult<String> {
        let secret_resource_id = secret_properties.resource_id().map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Invalid Azure Key Vault secret resource ID while listing secrets: {error}"
            ))
        })?;

        Ok(secret_resource_id.name)
    }
}

impl SettingsProvider for AzureKeyVaultSettingsProvider {
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        let data = self.data.load();
        data.clone_ref(py)
    }

    async fn load(&self) -> PyResult<()> {
        let secret_client = self.create_secret_client()?;
        let mut secret_properties_pager =
            secret_client
                .list_secret_properties(None)
                .map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to list secrets in Azure Key Vault '{}': {error}",
                        self.url
                    ))
                })?;
        let mut secret_values = BTreeMap::new();

        while let Some(secret_properties) =
            secret_properties_pager.try_next().await.map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to iterate secrets in Azure Key Vault '{}': {error}",
                    self.url
                ))
            })?
        {
            if !Self::is_secret_enabled(&secret_properties) {
                continue;
            }

            let secret_name = Self::extract_secret_name(&secret_properties)?;
            let secret_response =
                secret_client
                    .get_secret(&secret_name, None)
                    .await
                    .map_err(|error| {
                        PyRuntimeError::new_err(format!(
                            "Failed to read secret '{}' from Azure Key Vault '{}': {error}",
                            secret_name, self.url
                        ))
                    })?;
            let secret = secret_response.into_model().map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to deserialize Azure Key Vault secret '{secret_name}': {error}",
                ))
            })?;

            if let Some(secret_value) = secret.value {
                secret_values.insert(secret_name, Some(secret_value));
            }
        }

        self.normalize_keys(&mut secret_values);
        Python::attach(|py| -> PyResult<()> {
            let data = PyDict::new(py);

            for (secret_name, secret_value) in secret_values {
                data.set_item(secret_name, secret_value)?;
            }

            self.data.store(Arc::new(data.unbind()));
            Ok(())
        })?;
        Ok(())
    }

    fn section_separator() -> Option<&'static str> {
        Some("--")
    }
}

impl fmt::Display for AzureKeyVaultSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {{url: {}}}", self.get_type_name(), self.url)
    }
}

#[cfg(test)]
mod tests {
    use super::AzureKeyVaultSettingsProvider;
    use crate::core::SettingsProvider;
    use azure_security_keyvault_secrets::models::{SecretAttributes, SecretProperties};
    use pyo3::Python;

    #[test]
    fn test_replace_double_dash_with_dot_in_secret_name() {
        let normalized_key = AzureKeyVaultSettingsProvider::normalize_section_separator(
            String::from("Logging--LogLevel--Default"),
        );

        assert_eq!(normalized_key, "Logging.LogLevel.Default");
    }

    #[test]
    fn test_display_type() {
        Python::initialize();

        Python::attach(|py| {
            let url = String::from("https://example.vault.azure.net");
            let expected_display = format!("AzureKeyVaultSettingsProvider {{url: {url}}}");
            let display =
                AzureKeyVaultSettingsProvider::new(py, url, None, None, None, None).to_string();

            assert_eq!(display, expected_display);
        });
    }

    #[test]
    fn test_validate_explicit_credentials_require_all_fields() {
        Python::initialize();

        Python::attach(|py| {
            let provider = AzureKeyVaultSettingsProvider::new(
                py,
                String::from("https://example.vault.azure.net"),
                Some(String::from("client-id")),
                None,
                Some(String::from("tenant-id")),
                None,
            );

            let error = provider.validate_explicit_credentials().unwrap_err();

            assert_eq!(
                error.to_string(),
                "RuntimeError: 'tenant_id', 'client_id', and 'client_secret' must all be provided when using explicit Azure credentials"
            );
        });
    }

    #[test]
    fn test_detect_secret_enabled_when_attribute_is_true() {
        let mut secret_properties = SecretProperties::default();
        secret_properties.attributes = Some(SecretAttributes {
            enabled: Some(true),
            ..Default::default()
        });

        assert!(AzureKeyVaultSettingsProvider::is_secret_enabled(
            &secret_properties
        ));
    }

    #[test]
    fn test_detect_secret_not_enabled_when_attribute_is_false() {
        let mut secret_properties = SecretProperties::default();
        secret_properties.attributes = Some(SecretAttributes {
            enabled: Some(false),
            ..Default::default()
        });

        assert!(!AzureKeyVaultSettingsProvider::is_secret_enabled(
            &secret_properties
        ));
    }

    #[test]
    fn test_detect_secret_not_enabled_when_attribute_is_missing() {
        let secret_properties = SecretProperties::default();

        assert!(!AzureKeyVaultSettingsProvider::is_secret_enabled(
            &secret_properties
        ));
    }
}
