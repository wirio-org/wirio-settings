use arc_swap::ArcSwap;
use azure_identity::ClientSecretCredential;
use azure_security_keyvault_secrets::SecretClientOptions;
use azure_security_keyvault_secrets::models::Secret;
use azure_security_keyvault_secrets::{ResourceExt, SecretClient, models::SecretProperties};
use futures::TryStreamExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::azure_key_vault::default_azure_credential::DefaultAzureCredential;
use crate::azure_key_vault::remove_user_agent::RemoveUserAgent;
use crate::core::{PythonSettingsProvider, SettingLookup, SettingsProvider};

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct AzureKeyVaultSettingsProvider {
    secrets_cache: Arc<ArcSwap<SecretsCache>>,
    url: String,
    secret_client: Arc<SecretClient>,
    reload_interval: Option<Duration>,
    schedule_reload_cancellation_token: Mutex<Option<CancellationToken>>,
    schedule_reload_handle: Mutex<Option<JoinHandle<()>>>,
}

struct SecretsCache {
    data: Py<PyDict>,
    loaded_secrets: Option<BTreeMap<String, Secret>>,
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
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(Self::new(
                py,
                url,
                tenant_id,
                client_id,
                client_secret,
                reload_interval,
            )?),
        )
    }

    #[pyo3(signature = () -> "dict[str, str | None]")]
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        SettingsProvider::data(self, py)
    }

    fn try_get(&self, py: Python<'_>, key: &str) -> PyResult<SettingLookup> {
        SettingsProvider::try_get(self, py, key)
    }

    pub fn load(&self, py: Python<'_>) -> PyResult<()> {
        SettingsProvider::load(self, py)?;
        self.schedule_reload(py, self.reload_interval);
        Ok(())
    }
}

impl AzureKeyVaultSettingsProvider {
    pub fn new(
        py: Python<'_>,
        url: String,
        tenant_id: Option<String>,
        client_id: Option<String>,
        client_secret: Option<String>,
        reload_interval: Option<Duration>,
    ) -> PyResult<Self> {
        if let Some(reload_interval) = reload_interval
            && reload_interval.is_zero()
        {
            return Err(PyRuntimeError::new_err(
                "'reload_interval' must be greater than zero",
            ));
        }

        let secret_client = Self::create_secret_client(&url, tenant_id, client_id, client_secret)?;
        Ok(Self {
            secrets_cache: Arc::new(ArcSwap::from_pointee(SecretsCache {
                data: PyDict::new(py).unbind(),
                loaded_secrets: None,
            })),
            url,
            secret_client: Arc::new(secret_client),
            reload_interval,
            schedule_reload_cancellation_token: Mutex::new(None),
            schedule_reload_handle: Mutex::new(None),
        })
    }

    fn create_secret_client(
        url: &str,
        tenant_id: Option<String>,
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> PyResult<SecretClient> {
        Self::validate_explicit_credentials(
            tenant_id.as_deref(),
            client_id.as_deref(),
            client_secret.as_deref(),
        )?;

        if Self::has_explicit_credentials(
            tenant_id.as_deref(),
            client_id.as_deref(),
            client_secret.as_deref(),
        ) {
            let tenant_id = tenant_id.ok_or_else(|| {
                PyRuntimeError::new_err("Missing 'tenant_id' for explicit Azure credentials")
            })?;
            let client_id = client_id.ok_or_else(|| {
                PyRuntimeError::new_err("Missing 'client_id' for explicit Azure credentials")
            })?;
            let client_secret = client_secret.ok_or_else(|| {
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

            return SecretClient::new(url, credential, Some(secret_client_options)).map_err(
                |error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to create Azure Key Vault client for '{url}': {error}",
                    ))
                },
            );
        }

        let credential = DefaultAzureCredential::new();
        SecretClient::new(url, credential, None).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to create Azure Key Vault client for '{url}': {error}",
            ))
        })
    }

    fn validate_explicit_credentials(
        tenant_id: Option<&str>,
        client_id: Option<&str>,
        client_secret: Option<&str>,
    ) -> PyResult<()> {
        if Self::has_explicit_credentials(tenant_id, client_id, client_secret)
            && (tenant_id.is_none() || client_id.is_none() || client_secret.is_none())
        {
            return Err(PyRuntimeError::new_err(
                "'tenant_id', 'client_id', and 'client_secret' must all be provided when using explicit Azure credentials",
            ));
        }

        Ok(())
    }

    fn has_explicit_credentials(
        tenant_id: Option<&str>,
        client_id: Option<&str>,
        client_secret: Option<&str>,
    ) -> bool {
        tenant_id.is_some() || client_id.is_some() || client_secret.is_some()
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

    fn update_secrets(
        secrets_cache: &ArcSwap<SecretsCache>,
        new_loaded_secrets: BTreeMap<String, Secret>,
    ) -> PyResult<()> {
        let mut secret_values = new_loaded_secrets
            .iter()
            .map(|(secret_name, secret)| (secret_name.clone(), secret.value.clone()))
            .collect::<BTreeMap<String, Option<String>>>();
        Self::normalize_keys(&mut secret_values);

        Python::attach(|py| -> PyResult<()> {
            let data = PyDict::new(py);

            for (secret_name, secret_value) in secret_values {
                data.set_item(secret_name, secret_value)?;
            }

            secrets_cache.store(Arc::new(SecretsCache {
                data: data.unbind(),
                loaded_secrets: Some(new_loaded_secrets),
            }));
            Ok(())
        })
    }

    fn schedule_reload(&self, py: Python<'_>, reload_interval: Option<Duration>) {
        let Some(reload_interval) = reload_interval else {
            return;
        };

        py.detach(|| {
            let runtime = pyo3_async_runtimes::tokio::get_runtime();
            let cancellation_token = CancellationToken::new();
            self.schedule_reload_cancellation_token
                .blocking_lock()
                .replace(cancellation_token.clone());
            let secret_client = Arc::clone(&self.secret_client);
            let secrets_cache = Arc::clone(&self.secrets_cache);
            let url = self.url.clone();

            let schedule_reload_handle = runtime.spawn(async move {
                loop {
                    tokio::select! {
                        () = cancellation_token.cancelled() => break,
                        () = tokio::time::sleep(reload_interval) => {}
                    };
                    tokio::select! {
                        () = cancellation_token.cancelled() => break,
                        _ = Self::reload_secrets(
                            Arc::clone(&secret_client),
                            Arc::clone(&secrets_cache),
                            &url,
                        ) => {
                            // Ignore errors during scheduled reloads
                        }
                    };
                }
            });

            self.schedule_reload_handle
                .blocking_lock()
                .replace(schedule_reload_handle);
        });
    }

    async fn reload_secrets(
        secret_client: Arc<SecretClient>,
        secrets_cache: Arc<ArcSwap<SecretsCache>>,
        url: &str,
    ) -> PyResult<()> {
        let mut secret_properties_pager =
            secret_client
                .list_secret_properties(None)
                .map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to list secrets in Azure Key Vault '{url}': {error}",
                    ))
                })?;
        let mut new_loaded_secrets: BTreeMap<String, Secret> = BTreeMap::new();

        while let Some(secret_properties) =
            secret_properties_pager.try_next().await.map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to iterate secrets in Azure Key Vault '{url}': {error}",
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
                            "Failed to read secret '{secret_name}' from Azure Key Vault '{url}': {error}",
                        ))
                    })?;
            let secret = secret_response.into_model().map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to deserialize Azure Key Vault secret '{secret_name}': {error}",
                ))
            })?;
            new_loaded_secrets.insert(secret_name, secret);
        }

        Self::update_secrets(&secrets_cache, new_loaded_secrets)
    }
}

impl Drop for AzureKeyVaultSettingsProvider {
    fn drop(&mut self) {
        let schedule_reload_cancellation_token = self
            .schedule_reload_cancellation_token
            .blocking_lock()
            .take();

        if let Some(cancellation_token) = schedule_reload_cancellation_token {
            cancellation_token.cancel();
        }
    }
}

impl SettingsProvider for AzureKeyVaultSettingsProvider {
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        let secrets_cache = self.secrets_cache.load();
        secrets_cache.data.clone_ref(py)
    }

    async fn reload(&self) -> PyResult<()> {
        Self::reload_secrets(
            Arc::clone(&self.secret_client),
            Arc::clone(&self.secrets_cache),
            &self.url,
        )
        .await
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
    use std::time::Duration;

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
            let display = AzureKeyVaultSettingsProvider::new(py, url, None, None, None, None)
                .unwrap()
                .to_string();

            assert_eq!(display, expected_display);
        });
    }

    #[test]
    fn test_validate_explicit_credentials_require_all_fields() {
        let error = AzureKeyVaultSettingsProvider::validate_explicit_credentials(
            Some("tenant-id"),
            Some("client-id"),
            None,
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "RuntimeError: 'tenant_id', 'client_id', and 'client_secret' must all be provided when using explicit Azure credentials"
        );
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

    #[test]
    fn test_fail_creating_provider_when_reload_interval_is_zero() {
        Python::initialize();

        Python::attach(|py| {
            let result = AzureKeyVaultSettingsProvider::new(
                py,
                String::from("https://example.vault.azure.net"),
                None,
                None,
                None,
                Some(Duration::ZERO),
            );

            assert!(result.is_err());
            assert_eq!(
                result.err().unwrap().to_string(),
                "RuntimeError: 'reload_interval' must be greater than zero"
            );
        });
    }

    #[test]
    fn test_allow_creating_provider_when_reload_interval_is_positive() {
        Python::initialize();

        let result = Python::attach(|py| {
            AzureKeyVaultSettingsProvider::new(
                py,
                String::from("https://example.vault.azure.net"),
                None,
                None,
                None,
                Some(Duration::from_secs(1)),
            )
        });

        assert!(result.is_ok());
    }

    #[test]
    fn test_skip_scheduling_reload_when_interval_is_missing() {
        Python::initialize();
        Python::attach(|py| {
            let provider = AzureKeyVaultSettingsProvider::new(
                py,
                String::from("https://example.vault.azure.net"),
                None,
                None,
                None,
                None,
            )
            .unwrap();

            provider.schedule_reload(py, None);

            assert!(
                provider
                    .schedule_reload_cancellation_token
                    .blocking_lock()
                    .is_none()
            );
            assert!(provider.schedule_reload_handle.blocking_lock().is_none());
        });
    }

    #[test]
    fn test_cancel_scheduled_reload_when_provider_is_dropped() {
        Python::initialize();

        Python::attach(|py| {
            let provider = AzureKeyVaultSettingsProvider::new(
                py,
                String::from("https://example.vault.azure.net"),
                None,
                None,
                None,
                Some(Duration::from_secs(1)),
            )
            .unwrap();

            provider.schedule_reload(py, provider.reload_interval);

            let cancellation_token = provider
                .schedule_reload_cancellation_token
                .blocking_lock()
                .clone()
                .unwrap();
            drop(provider);

            assert!(cancellation_token.is_cancelled());
        });
    }
}
