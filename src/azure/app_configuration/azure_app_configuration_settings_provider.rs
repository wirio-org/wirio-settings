use crate::{
    azure::app_configuration::{
        azure_app_configuration_client::AzureAppConfigurationClient, dtos::Configuration,
        feature_management_input::FeatureManagementInput,
        parallel_azure_key_vault_reference_loader::ParallelAzureKeyVaultReferenceLoader,
    },
    core::{
        ModelRegistry, PythonSettingsProvider, SettingLookup, SettingsProvider,
        content_type::ContentType, convention_changer,
    },
};
use arc_swap::ArcSwap;
use azure_core::{credentials::TokenCredential, http::Url};
#[cfg(test)]
use azure_security_keyvault_secrets::SecretClientOptions;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::OnceCell;

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct AzureAppConfigurationSettingsProvider {
    data: ArcSwap<Py<PyDict>>,
    endpoint: String,
    client: Arc<AzureAppConfigurationClient>,
    credential: Arc<dyn TokenCredential>,
    model_registry: OnceCell<Py<ModelRegistry>>,
    #[cfg(test)]
    key_vault_client_options: SecretClientOptions,
}

#[pymethods]
impl AzureAppConfigurationSettingsProvider {
    #[pyo3(signature = () -> "dict[str, str | None]")]
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        SettingsProvider::data(self, py)
    }

    fn try_get(&self, py: Python<'_>, key: &str) -> PyResult<SettingLookup> {
        SettingsProvider::try_get(self, py, key)
    }

    pub fn load(&self, py: Python<'_>) -> PyResult<()> {
        SettingsProvider::load(self, py)
    }

    fn set_model_registry(&self, model_registry: PyRef<'_, ModelRegistry>) -> PyResult<()> {
        SettingsProvider::set_model_registry(self, model_registry)
    }
}

impl AzureAppConfigurationSettingsProvider {
    const FEATURE_MANAGEMENT_KEY: &str = "feature_management";
    pub(crate) fn new(
        py: Python<'_>,
        endpoint: String,
        client: Arc<AzureAppConfigurationClient>,
        credential: Arc<dyn TokenCredential>,
    ) -> Self {
        Self {
            data: ArcSwap::from_pointee(PyDict::new(py).unbind()),
            endpoint,
            client,
            credential,
            model_registry: OnceCell::new(),
            #[cfg(test)]
            key_vault_client_options: SecretClientOptions::default(),
        }
    }

    async fn add_configurations(
        &self,
        settings: &mut BTreeMap<String, Option<String>>,
    ) -> PyResult<()> {
        let configurations = self.client.get_configurations().await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to get configurations from Azure App Configuration '{endpoint}': {error}",
                endpoint = self.endpoint
            ))
        })?;
        Self::add_key_value_configurations(settings, &configurations);
        self.add_key_vault_reference_configurations(settings, configurations)
            .await?;
        Self::normalize_keys(settings);
        Ok(())
    }

    fn add_key_value_configurations(
        settings: &mut BTreeMap<String, Option<String>>,
        configurations: &[crate::azure::app_configuration::dtos::Configuration],
    ) {
        for configuration in configurations {
            let is_key_value = configuration
                .content_type
                .as_deref()
                .is_none_or(str::is_empty);

            if is_key_value {
                settings.insert(configuration.key.clone(), Some(configuration.value.clone()));
            }
        }
    }

    async fn add_key_vault_reference_configurations(
        &self,
        settings: &mut BTreeMap<String, Option<String>>,
        configurations: Vec<Configuration>,
    ) -> PyResult<()> {
        #[cfg(test)]
        let mut key_vault_reference_loader =
            ParallelAzureKeyVaultReferenceLoader::with_client_options(
                Arc::clone(&self.credential),
                self.key_vault_client_options.clone(),
            );
        #[cfg(not(test))]
        let mut key_vault_reference_loader =
            ParallelAzureKeyVaultReferenceLoader::new(Arc::clone(&self.credential));

        for configuration in configurations {
            let is_key_vault_reference =
                configuration
                    .content_type
                    .as_deref()
                    .is_some_and(|content_type| {
                        ContentType::new(content_type).is_key_vault_reference()
                    });

            if is_key_vault_reference {
                let secret_reference_uri = Self::extract_secret_reference_uri(&configuration)?;
                key_vault_reference_loader.add_reference(configuration.key, secret_reference_uri);
            }
        }

        let loaded_secrets = key_vault_reference_loader
            .load_all_secrets()
            .await?
            .into_iter()
            .map(|(configuration_key, secret)| (configuration_key, secret.value));
        settings.extend(loaded_secrets);
        Ok(())
    }

    fn extract_secret_reference_uri(configuration: &Configuration) -> PyResult<Url> {
        let secret_reference: KeyVaultReference = serde_json::from_str(&configuration.value)
            .map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Invalid Azure Key Vault reference for Azure App Configuration key '{}': {error}",
                    configuration.key,
                ))
            })?;

        Url::parse(&secret_reference.uri).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Invalid Azure Key Vault reference URI for Azure App Configuration key '{}': {error}",
                configuration.key,
            ))
        })
    }

    async fn add_enhanced_feature_flags(
        &self,
        settings: &mut BTreeMap<String, Option<String>>,
    ) -> PyResult<()> {
        let mut feature_flags = self.client.get_enhanced_feature_flags().await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to get enhanced feature flags from Azure App Configuration '{endpoint}': {error}",
                endpoint = self.endpoint
            ))
        })?;

        if !feature_flags.is_empty() {
            Self::normalize_enhanced_feature_flag_names(&mut feature_flags);
            let feature_management_input = FeatureManagementInput::from(feature_flags);
            let feature_management_input_json = serde_json::to_string(&feature_management_input)
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            settings.insert(
                String::from(Self::FEATURE_MANAGEMENT_KEY),
                Some(feature_management_input_json),
            );
        }

        Ok(())
    }

    fn normalize_enhanced_feature_flag_names(
        feature_flags: &mut [crate::azure::app_configuration::dtos::EnhancedFeatureFlag],
    ) {
        for feature_flag in feature_flags {
            feature_flag.name = convention_changer::to_snake_case(&feature_flag.name);
        }
    }

    #[cfg(test)]
    fn with_key_vault_client_options(
        mut self,
        key_vault_client_options: SecretClientOptions,
    ) -> Self {
        self.key_vault_client_options = key_vault_client_options;
        self
    }
}

impl SettingsProvider for AzureAppConfigurationSettingsProvider {
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        self.data.load().clone_ref(py)
    }

    async fn reload(&self) -> PyResult<()> {
        let mut settings = BTreeMap::new();
        self.add_configurations(&mut settings).await?;
        self.add_enhanced_feature_flags(&mut settings).await?;
        let data: Py<PyDict> = Python::attach(|py| Self::create_data(py, settings))?;
        self.data.store(Arc::new(data));
        Python::attach(|py| Self::on_reload(py, self.model_registry()));
        Ok(())
    }

    fn model_registry(&self) -> &OnceCell<Py<ModelRegistry>> {
        &self.model_registry
    }
}

impl fmt::Display for AzureAppConfigurationSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {{endpoint: {}}}",
            self.get_type_name(),
            self.endpoint
        )
    }
}

#[derive(Deserialize)]
struct KeyVaultReference {
    uri: String,
}

#[cfg(test)]
mod tests {
    use super::AzureAppConfigurationSettingsProvider;
    use crate::{
        azure::app_configuration::{
            azure_app_configuration_client::AzureAppConfigurationClient, dtos::Configuration,
            parallel_azure_key_vault_reference_loader::ParallelAzureKeyVaultReferenceLoader,
        },
        core::SettingsProvider,
    };
    use async_trait::async_trait;
    use azure_core::http::Url;
    use azure_core::{
        credentials::{AccessToken, TokenCredential, TokenRequestOptions},
        error::ErrorKind,
        http::{
            AsyncRawResponse, ClientOptions, HttpClient, Pipeline, Request, StatusCode, Transport,
            headers::Headers,
        },
    };
    use azure_security_keyvault_secrets::SecretClientOptions;
    use pyo3::{
        Python,
        types::{PyAnyMethods, PyDictMethods},
    };
    use std::sync::Arc;

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
                ErrorKind::Credential,
                "Test credential must not acquire a token",
            ))
        }
    }

    #[derive(Debug)]
    struct HttpClientMock {
        status_code: StatusCode,
    }

    #[async_trait]
    impl HttpClient for HttpClientMock {
        async fn execute_request(&self, request: &Request) -> azure_core::Result<AsyncRawResponse> {
            let response_body = match request.url().path() {
                "/kv" => br#"{"items": [
                    {"key": "ApplicationName", "value": "wirio"},
                    {"key": "Logging.LogLevel", "value": "warning"},
                    {
                        "key": "KeyVault1",
                        "content_type": "application/vnd.microsoft.appconfig.keyvaultref+json;charset=utf-8",
                        "value": "{\"uri\":\"https://example.vault.azure.net/secrets/Secret1\"}"
                    }
                ]}"#
                .as_slice(),
                "/ff" => br#"{"items": [{
                    "name": "Beta",
                    "enabled": true,
                    "conditions": {"requirement_type": "All", "filters": []},
                    "variants": [{
                        "name": "on",
                        "value": "{\"size\":500}",
                        "content_type": "application/json",
                        "status_override": "Disabled"
                    }],
                        "allocation": {"default_when_enabled": "on"},
                        "telemetry": {"enabled": true}
                }]}"#
                    .as_slice(),
                    path => panic!("Unexpected request path: {path}"),
            };

            Ok(AsyncRawResponse::from_bytes(
                self.status_code,
                Headers::default(),
                response_body,
            ))
        }
    }

    #[derive(Debug)]
    struct KeyVaultHttpClientMock;

    #[async_trait]
    impl HttpClient for KeyVaultHttpClientMock {
        async fn execute_request(&self, request: &Request) -> azure_core::Result<AsyncRawResponse> {
            let response_body = match (request.url().host_str(), request.url().path()) {
                (Some("example.vault.azure.net"), "/secrets/Secret1/") => {
                    br#"{"value":"secret1-value"}"#.to_vec()
                }
                (Some("example.vault.azure.net"), "/secrets/Secret2/version1") => {
                    br#"{"value":"secret2-value"}"#.to_vec()
                }
                (Some("another.vault.azure.net"), "/secrets/Secret3/") => {
                    br#"{"value":"secret3-value"}"#.to_vec()
                }
                (host, path) => panic!("Unexpected Key Vault request: {host:?}{path}"),
            };

            Ok(AsyncRawResponse::from_bytes(
                StatusCode::Ok,
                Headers::default(),
                response_body,
            ))
        }
    }

    fn create_provider(
        py: Python<'_>,
        status_code: StatusCode,
    ) -> AzureAppConfigurationSettingsProvider {
        let http_client_mock: Arc<dyn HttpClient> = Arc::new(HttpClientMock { status_code });
        let credential: Arc<dyn TokenCredential> = Arc::new(CredentialMock);
        let client = AzureAppConfigurationClient::new(
            "https://example.azconfig.io",
            Arc::clone(&credential),
        )
        .unwrap()
        .with_pipeline(Pipeline::new(
            option_env!("CARGO_PKG_NAME"),
            option_env!("CARGO_PKG_VERSION"),
            ClientOptions {
                transport: Some(Transport::new(Arc::clone(&http_client_mock))),
                ..Default::default()
            },
            Vec::new(),
            Vec::new(),
            None,
        ));

        AzureAppConfigurationSettingsProvider::new(
            py,
            String::from("https://example.azconfig.io"),
            Arc::new(client),
            credential,
        )
        .with_key_vault_client_options(SecretClientOptions {
            client_options: ClientOptions {
                transport: Some(Transport::new(Arc::new(KeyVaultHttpClientMock))),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn test_load_and_normalize_configurations_and_enhanced_feature_flags() {
        Python::initialize();
        let provider = Python::attach(|py| create_provider(py, StatusCode::Ok));

        provider.reload().await.unwrap();

        Python::attach(|py| {
            let data = provider.data(py);
            let data = data.bind(py);

            assert_eq!(data.len(), 4);
            assert_eq!(
                data.get_item("application_name")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "wirio"
            );
            assert_eq!(
                data.get_item("logging.log_level")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "warning"
            );
            assert_eq!(
                data.get_item("key_vault_1")
                    .unwrap()
                    .unwrap()
                    .extract::<String>()
                    .unwrap(),
                "secret1-value"
            );
            let feature_management_json = data
                .get_item(AzureAppConfigurationSettingsProvider::FEATURE_MANAGEMENT_KEY)
                .unwrap()
                .unwrap()
                .extract::<String>()
                .unwrap();
            let feature_management: serde_json::Value =
                serde_json::from_str(&feature_management_json).unwrap();
            let expected_feature_management: serde_json::Value = serde_json::from_str(
                r#"{"feature_management":{"feature_flags":[{"id":"beta","enabled":true,"conditions":{"requirement_type":"All","client_filters":[]},"variants":[{"name":"on","configuration_value":{"size":500},"status_override":"Disabled"}],"allocation":{"default_when_enabled":"on"},"telemetry":{"enabled":true}}]}}"#,
            )
            .unwrap();

            assert_eq!(feature_management, expected_feature_management);
        });
    }

    #[tokio::test]
    async fn test_fail_loading_configurations_when_response_status_code_is_unsuccessful() {
        Python::initialize();
        let provider = Python::attach(|py| create_provider(py, StatusCode::BadRequest));

        let error = provider.reload().await.unwrap_err();

        assert!(error.to_string().starts_with(
            "RuntimeError: Failed to get configurations from Azure App Configuration 'https://example.azconfig.io':"
        ));
    }

    #[test]
    fn test_display_includes_endpoint() {
        Python::initialize();
        let provider = Python::attach(|py| create_provider(py, StatusCode::Ok));

        assert_eq!(
            provider.to_string(),
            "AzureAppConfigurationSettingsProvider {endpoint: https://example.azconfig.io}"
        );
    }

    #[tokio::test]
    async fn test_load_key_vault_reference_value() {
        let configuration_name = String::from("configuration_to_key_vault_secret_2");
        let expected_secret_value = "secret2-value";
        let mut key_vault_reference_loader =
            ParallelAzureKeyVaultReferenceLoader::with_client_options(
                Arc::new(CredentialMock),
                SecretClientOptions {
                    client_options: ClientOptions {
                        transport: Some(Transport::new(Arc::new(KeyVaultHttpClientMock))),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
        let configuration = Configuration {
            key: configuration_name.clone(),
            content_type: Some(String::from(
                "application/vnd.microsoft.appconfig.keyvaultref+json;charset=utf-8",
            )),
            value: String::from(
                "{\"uri\":\"https://example.vault.azure.net/secrets/Secret2/version1\"}",
            ),
        };
        let secret_reference_uri =
            AzureAppConfigurationSettingsProvider::extract_secret_reference_uri(&configuration)
                .unwrap();

        key_vault_reference_loader.add_reference(configuration.key, secret_reference_uri);

        let loaded_values = key_vault_reference_loader.load_all_secrets().await.unwrap();

        assert_eq!(
            loaded_values
                .get(&configuration_name)
                .and_then(|secret| secret.value.as_deref()),
            Some(expected_secret_value)
        );
    }

    #[tokio::test]
    async fn test_load_key_vault_references_from_multiple_vaults() {
        let mut key_vault_reference_loader =
            ParallelAzureKeyVaultReferenceLoader::with_client_options(
                Arc::new(CredentialMock),
                SecretClientOptions {
                    client_options: ClientOptions {
                        transport: Some(Transport::new(Arc::new(KeyVaultHttpClientMock))),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
        key_vault_reference_loader.add_reference(
            String::from("first_key"),
            Url::parse("https://example.vault.azure.net/secrets/Secret1").unwrap(),
        );
        key_vault_reference_loader.add_reference(
            String::from("second_key"),
            Url::parse("https://example.vault.azure.net/secrets/Secret2/version1").unwrap(),
        );
        key_vault_reference_loader.add_reference(
            String::from("third_key"),
            Url::parse("https://another.vault.azure.net/secrets/Secret3").unwrap(),
        );

        let loaded_values = key_vault_reference_loader.load_all_secrets().await.unwrap();

        assert_eq!(
            loaded_values
                .get("first_key")
                .and_then(|secret| secret.value.as_deref()),
            Some("secret1-value")
        );
        assert_eq!(
            loaded_values
                .get("second_key")
                .and_then(|secret| secret.value.as_deref()),
            Some("secret2-value")
        );
        assert_eq!(
            loaded_values
                .get("third_key")
                .and_then(|secret| secret.value.as_deref()),
            Some("secret3-value")
        );
    }
}
