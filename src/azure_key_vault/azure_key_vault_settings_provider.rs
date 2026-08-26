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
}

struct SecretsCache {
    data: Py<PyDict>,
    loaded_secrets: Option<BTreeMap<String, Secret>>,
}

#[pymethods]
impl AzureKeyVaultSettingsProvider {
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

    /// Check if the loaded secret is up to date with the queried secret properties
    fn is_secret_up_to_date(loaded_secret: &Secret, secret_properties: &SecretProperties) -> bool {
        loaded_secret
            .attributes
            .as_ref()
            .and_then(|attributes| attributes.updated)
            == secret_properties
                .attributes
                .as_ref()
                .and_then(|attributes| attributes.updated)
    }

    async fn add_secret(
        secret_client: &SecretClient,
        loaded_secrets: Option<&BTreeMap<String, Secret>>,
        secret_properties: SecretProperties,
        new_loaded_secrets: &mut BTreeMap<String, Secret>,
        url: &str,
    ) -> PyResult<()> {
        if !Self::is_secret_enabled(&secret_properties) {
            return Ok(());
        }

        let secret_name = Self::extract_secret_name(&secret_properties)?;
        let loaded_secret =
            loaded_secrets.and_then(|loaded_secrets| loaded_secrets.get(&secret_name));

        if let Some(loaded_secret) = loaded_secret
            && Self::is_secret_up_to_date(loaded_secret, &secret_properties)
        {
            new_loaded_secrets.insert(secret_name, loaded_secret.clone());
            return Ok(());
        }

        let retrieved_secret = Self::retrieve_secret(secret_client, &secret_name, url).await?;
        new_loaded_secrets.insert(secret_name, retrieved_secret);
        Ok(())
    }

    async fn retrieve_secret(
        secret_client: &SecretClient,
        secret_name: &str,
        url: &str,
    ) -> PyResult<Secret> {
        let secret_response = secret_client.get_secret(secret_name, None).await.map_err(
            |error| {
                PyRuntimeError::new_err(format!(
                    "Failed to read secret '{secret_name}' from Azure Key Vault '{url}': {error}",
                ))
            },
        )?;
        secret_response.into_model().map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to deserialize Azure Key Vault secret '{secret_name}': {error}",
            ))
        })
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

            runtime.spawn(async move {
                loop {
                    tokio::select! {
                        () = cancellation_token.cancelled() => break,
                        () = tokio::time::sleep(reload_interval) => {}
                    };
                    tokio::select! {
                        () = cancellation_token.cancelled() => break,
                        _ = Self::reload_secrets(
                            &secret_client,
                            &secrets_cache,
                            &url,
                        ) => {
                            // Ignore errors during scheduled reloads
                        }
                    };
                }
            });
        });
    }

    async fn reload_secrets(
        secret_client: &SecretClient,
        secrets_cache: &ArcSwap<SecretsCache>,
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
        let loaded_secrets_cache = secrets_cache.load_full();
        let loaded_secrets = loaded_secrets_cache.loaded_secrets.as_ref();

        while let Some(secret_properties) =
            secret_properties_pager.try_next().await.map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to iterate secrets in Azure Key Vault '{url}': {error}",
                ))
            })?
        {
            Self::add_secret(
                secret_client,
                loaded_secrets,
                secret_properties,
                &mut new_loaded_secrets,
                url,
            )
            .await?;
        }

        Self::update_secrets(secrets_cache, new_loaded_secrets)
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
        let secret_client = Arc::clone(&self.secret_client);
        let secrets_cache = Arc::clone(&self.secrets_cache);
        Self::reload_secrets(&secret_client, &secrets_cache, &self.url).await
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
    use super::{AzureKeyVaultSettingsProvider, SecretsCache};
    use crate::core::SettingsProvider;
    use arc_swap::ArcSwap;
    use async_trait::async_trait;
    use azure_core::credentials::{AccessToken, TokenCredential, TokenRequestOptions};
    use azure_core::http::{
        AsyncRawResponse, ClientOptions, HttpClient, Request, StatusCode, Transport,
        headers::Headers,
    };
    use azure_security_keyvault_secrets::SecretClient;
    use azure_security_keyvault_secrets::SecretClientOptions;
    use azure_security_keyvault_secrets::models::{Secret, SecretAttributes, SecretProperties};
    use pyo3::Python;
    use pyo3::types::PyDict;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Debug)]
    struct CredentialMock;

    #[async_trait]
    impl TokenCredential for CredentialMock {
        async fn get_token(
            &self,
            _scopes: &[&str],
            _options: Option<TokenRequestOptions<'_>>,
        ) -> azure_core::Result<AccessToken> {
            Err(azure_core::Error::with_message(
                azure_core::error::ErrorKind::Credential,
                "Test credential must not acquire a token",
            ))
        }
    }

    #[derive(Debug)]
    struct HttpClientMock {
        response_body: Vec<u8>,
    }

    #[async_trait]
    impl HttpClient for HttpClientMock {
        async fn execute_request(
            &self,
            _request: &Request,
        ) -> azure_core::Result<AsyncRawResponse> {
            Ok(AsyncRawResponse::from_bytes(
                StatusCode::Ok,
                Headers::default(),
                self.response_body.clone(),
            ))
        }
    }

    fn create_secret_client_mock(url: &str) -> SecretClient {
        SecretClient::new(url, Arc::new(CredentialMock), None).unwrap()
    }

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

            py.detach(|| {
                assert!(
                    provider
                        .schedule_reload_cancellation_token
                        .blocking_lock()
                        .is_none()
                );
            });
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

    #[test]
    fn test_consider_secret_up_to_date_when_update_times_match() {
        let update_time = azure_core::time::OffsetDateTime::UNIX_EPOCH;
        let mut secret = Secret::default();
        secret.attributes = Some(SecretAttributes {
            updated: Some(update_time),
            ..Default::default()
        });
        let mut secret_properties = SecretProperties::default();
        secret_properties.attributes = Some(SecretAttributes {
            updated: Some(update_time),
            ..Default::default()
        });

        assert!(AzureKeyVaultSettingsProvider::is_secret_up_to_date(
            &secret,
            &secret_properties
        ));
    }

    #[test]
    fn test_consider_secret_up_to_date_when_update_times_are_missing() {
        let mut secret = Secret::default();
        secret.attributes = Some(SecretAttributes::default());
        let mut secret_properties = SecretProperties::default();
        secret_properties.attributes = Some(SecretAttributes::default());

        assert!(AzureKeyVaultSettingsProvider::is_secret_up_to_date(
            &secret,
            &secret_properties
        ));
    }

    #[test]
    fn test_consider_secret_outdated_when_update_times_differ() {
        let mut secret = Secret::default();
        secret.attributes = Some(SecretAttributes {
            updated: Some(azure_core::time::OffsetDateTime::UNIX_EPOCH),
            ..Default::default()
        });
        let mut secret_properties = SecretProperties::default();
        secret_properties.attributes = Some(SecretAttributes::default());

        assert!(!AzureKeyVaultSettingsProvider::is_secret_up_to_date(
            &secret,
            &secret_properties
        ));
    }

    #[tokio::test]
    async fn test_skip_disabled_secret_when_adding_secret() {
        let secret_client = create_secret_client_mock("https://example.vault.azure.net");
        let mut secret_properties = SecretProperties::default();
        secret_properties.attributes = Some(SecretAttributes {
            enabled: Some(false),
            ..Default::default()
        });
        let mut new_loaded_secrets = BTreeMap::new();

        AzureKeyVaultSettingsProvider::add_secret(
            &secret_client,
            None,
            secret_properties,
            &mut new_loaded_secrets,
            "https://example.vault.azure.net",
        )
        .await
        .unwrap();

        assert!(new_loaded_secrets.is_empty());
    }

    #[tokio::test]
    async fn test_reuse_cached_secret_when_adding_up_to_date_secret() {
        let expected_cache_value = "cached-value";
        let secret_client = create_secret_client_mock("https://example.vault.azure.net");
        let update_time = azure_core::time::OffsetDateTime::UNIX_EPOCH;
        let secret_name: String = String::from("cached-secret");
        let mut cached_secret = Secret::default();
        cached_secret.value = Some(expected_cache_value.to_owned());
        cached_secret.attributes = Some(SecretAttributes {
            updated: Some(update_time),
            ..Default::default()
        });
        let loaded_secrets = BTreeMap::from([(secret_name.clone(), cached_secret)]);
        let mut secret_properties = SecretProperties::default();
        secret_properties.id = Some(format!(
            "https://example.vault.azure.net/secrets/{secret_name}/version"
        ));
        secret_properties.attributes = Some(SecretAttributes {
            enabled: Some(true),
            updated: Some(update_time),
            ..Default::default()
        });
        let mut new_loaded_secrets = BTreeMap::new();

        AzureKeyVaultSettingsProvider::add_secret(
            &secret_client,
            Some(&loaded_secrets),
            secret_properties,
            &mut new_loaded_secrets,
            "https://example.vault.azure.net",
        )
        .await
        .unwrap();

        assert_eq!(
            new_loaded_secrets
                .get(&secret_name)
                .and_then(|secret| secret.value.as_deref()),
            Some(expected_cache_value)
        );
    }
    #[tokio::test]
    async fn test_retrieve_secret_when_cached_secret_is_missing() {
        let expected_secret_name = "missing-secret";
        let expected_secret_value = "retrieved-value";
        let url = "https://example.vault.azure.net";
        let secret_client_options = SecretClientOptions {
            client_options: ClientOptions {
                transport: Some(Transport::new(Arc::new(HttpClientMock {
                    response_body: br#"{"value":"retrieved-value"}"#.to_vec(),
                }))),
                ..Default::default()
            },
            ..Default::default()
        };
        let secret_client =
            SecretClient::new(url, Arc::new(CredentialMock), Some(secret_client_options)).unwrap();

        let mut secret_properties = SecretProperties::default();
        secret_properties.id = Some(format!(
            "https://example.vault.azure.net/secrets/{expected_secret_name}/version"
        ));
        secret_properties.attributes = Some(SecretAttributes {
            enabled: Some(true),
            ..Default::default()
        });
        let mut new_loaded_secrets = BTreeMap::new();

        AzureKeyVaultSettingsProvider::add_secret(
            &secret_client,
            None,
            secret_properties,
            &mut new_loaded_secrets,
            url,
        )
        .await
        .unwrap();

        assert_eq!(
            new_loaded_secrets
                .get(expected_secret_name)
                .and_then(|secret| secret.value.as_deref()),
            Some(expected_secret_value)
        );
    }

    #[tokio::test]
    async fn test_remove_cached_secret_when_is_missing() {
        Python::initialize();

        let cached_secret_name = String::from("deleted-secret");
        let mut cached_secret_value = Secret::default();
        cached_secret_value.value = Some(String::from("cached-value"));
        let secrets_cache = Python::attach(|py| {
            Arc::new(ArcSwap::from_pointee(SecretsCache {
                data: PyDict::new(py).unbind(),
                loaded_secrets: Some(BTreeMap::from([(
                    cached_secret_name.clone(),
                    cached_secret_value,
                )])),
            }))
        });
        let url = "https://example.vault.azure.net";
        let secret_client_options = SecretClientOptions {
            client_options: ClientOptions {
                transport: Some(Transport::new(Arc::new(HttpClientMock {
                    response_body: br#"{"value":[]}"#.to_vec(),
                }))),
                ..Default::default()
            },
            ..Default::default()
        };
        let secret_client = Arc::new(
            SecretClient::new(url, Arc::new(CredentialMock), Some(secret_client_options)).unwrap(),
        );

        AzureKeyVaultSettingsProvider::reload_secrets(&secret_client, &secrets_cache, url)
            .await
            .unwrap();

        let reloaded_secrets = secrets_cache.load_full();
        assert!(
            !reloaded_secrets
                .loaded_secrets
                .as_ref()
                .unwrap()
                .contains_key(&cached_secret_name)
        );
    }
}
