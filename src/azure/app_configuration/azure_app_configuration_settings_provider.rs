use crate::{
    azure::app_configuration::{
        ParallelAzureKeyVaultReferenceLoader,
        azure_app_configuration_client::AzureAppConfigurationClient,
        azure_app_configuration_settings_source::AzureAppConfigurationSettingsSource,
        dtos::{ConfigurationSetting, EnhancedFeatureFlag, KeyVaultReference, SnapshotReference},
        feature_management_input::FeatureManagementInput,
        models::{FeatureFlagSelector, SettingSelector},
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
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::OnceCell;

#[pyclass(
    name = "_AzureAppConfigurationSettingsProvider",
    extends = PythonSettingsProvider,
    frozen,
    str
)]
pub struct AzureAppConfigurationSettingsProvider {
    data: ArcSwap<Py<PyDict>>,
    endpoint: String,
    client: Arc<AzureAppConfigurationClient>,
    credential: Arc<dyn TokenCredential>,
    selectors: Vec<SettingSelector>,
    trim_key_prefixes: Vec<String>,
    feature_flag_selectors: Vec<FeatureFlagSelector>,
    feature_flag_trim_name_prefixes: Vec<String>,
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

    pub(crate) fn new(py: Python<'_>, source: &AzureAppConfigurationSettingsSource) -> Self {
        Self {
            data: ArcSwap::from_pointee(PyDict::new(py).unbind()),
            endpoint: source.endpoint.clone(),
            client: Arc::clone(&source.client),
            credential: Arc::clone(&source.credential),
            selectors: source.selectors.clone(),
            trim_key_prefixes: source.trim_key_prefixes.clone(),
            feature_flag_selectors: source.feature_flag_selectors.clone(),
            feature_flag_trim_name_prefixes: source.feature_flag_trim_name_prefixes.clone(),
            model_registry: OnceCell::new(),
            #[cfg(test)]
            key_vault_client_options: SecretClientOptions::default(),
        }
    }

    async fn add_configuration_settings(
        &self,
        settings: &mut BTreeMap<String, Option<String>>,
    ) -> PyResult<()> {
        let mut configuration_settings: Vec<ConfigurationSetting> = Vec::new();

        for selector in &self.selectors {
            let selector_configurations =
                self.client.get_configuration_settings(selector).await.map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to get configuration settings from Azure App Configuration '{endpoint}': {error}",
                        endpoint = self.endpoint
                    ))
                })?;
            let selector_configurations = self
                .resolve_snapshot_references(selector_configurations)
                .await?;
            configuration_settings.extend(selector_configurations);
        }
        Self::add_key_value_configuration_settings(settings, &configuration_settings);
        self.add_key_vault_reference_configuration_settings(settings, configuration_settings)
            .await?;
        Self::trim_configuration_setting_key_prefixes(settings, &self.trim_key_prefixes);
        Self::normalize_keys(settings);
        Ok(())
    }

    async fn resolve_snapshot_references(
        &self,
        configuration_settings: Vec<ConfigurationSetting>,
    ) -> PyResult<Vec<ConfigurationSetting>> {
        let mut resolved_configuration_settings = Vec::new();

        for configuration_setting in configuration_settings {
            let is_snapshot_reference = configuration_setting
                .content_type
                .as_deref()
                .is_some_and(|content_type| ContentType::new(content_type).is_snapshot_reference());

            if !is_snapshot_reference {
                resolved_configuration_settings.push(configuration_setting);
                continue;
            }

            let snapshot_reference: SnapshotReference =
                serde_json::from_str(&configuration_setting.value).map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Invalid Azure App Configuration snapshot reference for key '{}': {error}",
                        configuration_setting.key,
                    ))
                })?;
            let selector = SettingSelector::new(None, None, Some(snapshot_reference.snapshot_name))
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let snapshot_configuration_settings = self
                .client
                .get_configuration_settings(&selector)
                .await
                .map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to get snapshot reference from Azure App Configuration '{endpoint}': {error}",
                        endpoint = self.endpoint,
                    ))
                })?;
            resolved_configuration_settings.extend(snapshot_configuration_settings);
        }

        Ok(resolved_configuration_settings)
    }

    fn add_key_value_configuration_settings(
        settings: &mut BTreeMap<String, Option<String>>,
        configuration_settings: &[ConfigurationSetting],
    ) {
        for configuration_setting in configuration_settings {
            let is_key_value = configuration_setting
                .content_type
                .as_deref()
                .is_none_or(str::is_empty);

            if is_key_value {
                settings.insert(
                    configuration_setting.key.clone(),
                    Some(configuration_setting.value.clone()),
                );
            }
        }
    }

    async fn add_key_vault_reference_configuration_settings(
        &self,
        settings: &mut BTreeMap<String, Option<String>>,
        configuration_settings: Vec<ConfigurationSetting>,
    ) -> PyResult<()> {
        let mut key_vault_reference_loader =
            ParallelAzureKeyVaultReferenceLoader::new(Arc::clone(&self.credential));

        #[cfg(test)]
        key_vault_reference_loader.with_client_options(self.key_vault_client_options.clone());

        let mut secret_references = BTreeMap::new();

        for configuration_setting in configuration_settings {
            let is_key_vault_reference =
                configuration_setting
                    .content_type
                    .as_deref()
                    .is_some_and(|content_type| {
                        ContentType::new(content_type).is_key_vault_reference()
                    });

            if is_key_vault_reference {
                let secret_reference_uri =
                    Self::extract_secret_reference_uri(&configuration_setting)?;
                secret_references.insert(configuration_setting.key, secret_reference_uri);
            }
        }

        for (configuration_setting_key, secret_reference_uri) in secret_references {
            key_vault_reference_loader
                .add_reference(configuration_setting_key, secret_reference_uri);
        }

        let loaded_secrets = key_vault_reference_loader
            .load_all_secrets()
            .await?
            .into_iter()
            .map(|(configuration_setting_key, secret)| (configuration_setting_key, secret.value));
        settings.extend(loaded_secrets);
        Ok(())
    }

    fn extract_secret_reference_uri(configuration_setting: &ConfigurationSetting) -> PyResult<Url> {
        let secret_reference: KeyVaultReference = serde_json::from_str(
            &configuration_setting.value,
        )
        .map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Invalid Azure Key Vault reference for Azure App Configuration key '{}': {error}",
                configuration_setting.key,
            ))
        })?;

        Url::parse(&secret_reference.uri).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Invalid Azure Key Vault reference URI for Azure App Configuration key '{}': {error}",
                configuration_setting.key,
            ))
        })
    }

    async fn add_enhanced_feature_flags(
        &self,
        settings: &mut BTreeMap<String, Option<String>>,
    ) -> PyResult<()> {
        let mut feature_flags = BTreeMap::new();

        for selector in &self.feature_flag_selectors {
            let selector_feature_flags = self
                .client
                .get_enhanced_feature_flags(selector)
                .await
                .map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "Failed to get enhanced feature flags from Azure App Configuration '{endpoint}': {error}",
                        endpoint = self.endpoint
                    ))
                })?;
            feature_flags.extend(
                selector_feature_flags
                    .into_iter()
                    .map(|feature_flag| (feature_flag.name.clone(), feature_flag)),
            );
        }

        if !feature_flags.is_empty() {
            let mut feature_flags: Vec<_> = feature_flags.into_values().collect();
            Self::trim_feature_flag_name_prefixes(
                &mut feature_flags,
                &self.feature_flag_trim_name_prefixes,
            );
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

    fn normalize_enhanced_feature_flag_names(feature_flags: &mut [EnhancedFeatureFlag]) {
        for feature_flag in feature_flags {
            feature_flag.name = convention_changer::to_snake_case(&feature_flag.name);
        }
    }

    fn trim_configuration_setting_key_prefixes(
        settings: &mut BTreeMap<String, Option<String>>,
        prefixes: &[String],
    ) {
        if prefixes.is_empty() {
            return;
        }

        let trimmed_settings = std::mem::take(settings).into_iter().map(|(key, value)| {
            let new_key = Self::trim_prefix(key, prefixes);
            (new_key, value)
        });
        settings.extend(trimmed_settings);
    }

    fn trim_feature_flag_name_prefixes(
        feature_flags: &mut [EnhancedFeatureFlag],
        prefixes: &[String],
    ) {
        if prefixes.is_empty() {
            return;
        }

        for feature_flag in feature_flags {
            let feature_flag_name = std::mem::take(&mut feature_flag.name);
            let new_feature_flag_name = Self::trim_prefix(feature_flag_name, prefixes);
            feature_flag.name = new_feature_flag_name;
        }
    }

    fn trim_prefix(value: String, prefixes: &[String]) -> String {
        for prefix in prefixes {
            if let Some(trimmed_value) = value.strip_prefix(prefix) {
                return String::from(trimmed_value);
            }
        }

        value
    }
}

impl SettingsProvider for AzureAppConfigurationSettingsProvider {
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        self.data.load().clone_ref(py)
    }

    async fn refresh(&self) -> PyResult<()> {
        let mut settings = BTreeMap::new();
        self.add_configuration_settings(&mut settings).await?;
        self.add_enhanced_feature_flags(&mut settings).await?;
        let data: Py<PyDict> = Python::attach(|py| Self::create_data(py, settings))?;
        self.data.store(Arc::new(data));
        Python::attach(|py| {
            Self::on_refresh(py, self.model_registry());
            log::info!(
                "Loaded settings from Azure App Configuration endpoint '{}'",
                self.endpoint
            );
        });
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

#[cfg(test)]
mod tests {
    use super::AzureAppConfigurationSettingsProvider;
    use crate::{
        azure::app_configuration::{
            azure_app_configuration_client::AzureAppConfigurationClient,
            dtos::{ConfigurationSetting, EnhancedFeatureFlag},
            models::{FeatureFlagSelector, SettingSelector},
            parallel_azure_key_vault_reference_loader::ParallelAzureKeyVaultReferenceLoader,
        },
        core::SettingsProvider,
    };
    use arc_swap::ArcSwap;
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
        types::{PyAnyMethods, PyDict, PyDictMethods},
    };
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use tokio::sync::OnceCell;

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
            let response_body = match (request.url().path(), request.url().query()) {
                ("/snapshots/production", _) => br#"{"name":"production"}"#.as_slice(),
                ("/kv", Some(query)) if query.contains("snapshot=production") => br#"{"items": [
                    {"key": "ApplicationName", "value": "wirio"},
                    {"key": "Logging.LogLevel", "value": "warning"}
                ]}"#
                .as_slice(),
                ("/kv", _) => br#"{"items": [
                    {"key": "ApplicationName", "value": "wirio"},
                    {"key": "Logging.LogLevel", "value": "warning"},
                    {
                        "key": "KeyVault1",
                        "content_type": "application/vnd.microsoft.appconfig.keyvaultref+json;charset=utf-8",
                        "value": "{\"uri\":\"https://example.vault.azure.net/secrets/Secret1\"}"
                    }
                ]}"#
                .as_slice(),
                ("/ff", _) if request
                    .url()
                    .query()
                    .is_some_and(|query| query.contains("label=Production")) => {
                    br#"{"items": [{"name": "Beta", "enabled": false}]}"#.as_slice()
                }
                ("/ff", _) => br#"{"items": [{
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
                (path, _) => panic!("Unexpected request path: {path}"),
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
        create_provider_with_feature_flag_selectors(
            py,
            status_code,
            vec![FeatureFlagSelector::new(String::from("*"), None)],
        )
    }

    fn create_provider_with_feature_flag_selectors(
        py: Python<'_>,
        status_code: StatusCode,
        feature_flag_selectors: Vec<FeatureFlagSelector>,
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

        AzureAppConfigurationSettingsProvider {
            data: ArcSwap::from_pointee(PyDict::new(py).unbind()),
            endpoint: String::from("https://example.azconfig.io"),
            client: Arc::new(client),
            credential,
            selectors: vec![SettingSelector::new(Some(String::from("*")), None, None).unwrap()],
            trim_key_prefixes: Vec::new(),
            feature_flag_selectors,
            feature_flag_trim_name_prefixes: Vec::new(),
            model_registry: OnceCell::new(),
            key_vault_client_options: SecretClientOptions {
                client_options: ClientOptions {
                    transport: Some(Transport::new(Arc::new(KeyVaultHttpClientMock))),
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }

    #[tokio::test]
    async fn test_load_and_normalize_configuration_settings_and_enhanced_feature_flags() {
        Python::initialize();
        let provider = Python::attach(|py| create_provider(py, StatusCode::Ok));

        provider.refresh().await.unwrap();

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

    #[test]
    fn test_trim_prefixes() {
        let expected_application_name = "wirio";
        let expected_logging_level = "warning";
        let mut configuration_settings = BTreeMap::from([
            (
                String::from("service:application_name"),
                Some(String::from(expected_application_name)),
            ),
            (
                String::from("logging.log_level"),
                Some(String::from(expected_logging_level)),
            ),
        ]);
        let mut feature_flags = vec![EnhancedFeatureFlag {
            name: String::from("service:beta_feature"),
            enabled: true,
            description: None,
            conditions: None,
            variants: None,
            allocation: None,
            telemetry: None,
        }];

        AzureAppConfigurationSettingsProvider::trim_configuration_setting_key_prefixes(
            &mut configuration_settings,
            &[String::from("service:")],
        );
        AzureAppConfigurationSettingsProvider::trim_feature_flag_name_prefixes(
            &mut feature_flags,
            &[String::from("service:")],
        );

        assert_eq!(
            configuration_settings.get("application_name"),
            Some(&Some(String::from(expected_application_name)))
        );
        assert_eq!(
            configuration_settings.get("logging.log_level"),
            Some(&Some(String::from(expected_logging_level)))
        );
        assert_eq!(feature_flags[0].name, "beta_feature");
    }

    #[tokio::test]
    async fn test_fail_loading_configuration_settings_when_response_status_code_is_unsuccessful() {
        Python::initialize();
        let provider = Python::attach(|py| create_provider(py, StatusCode::BadRequest));

        let error = provider.refresh().await.unwrap_err();

        assert!(error.to_string().starts_with(
            "RuntimeError: Failed to get configuration settings from Azure App Configuration 'https://example.azconfig.io':"
        ));
    }

    #[tokio::test]
    async fn test_use_feature_flag_from_later_selector() {
        Python::initialize();
        let provider = Python::attach(|py| {
            create_provider_with_feature_flag_selectors(
                py,
                StatusCode::Ok,
                vec![
                    FeatureFlagSelector::new(String::from("*"), None),
                    FeatureFlagSelector::new(String::from("*"), Some(String::from("Production"))),
                ],
            )
        });
        let mut settings = BTreeMap::new();

        provider
            .add_enhanced_feature_flags(&mut settings)
            .await
            .unwrap();

        let feature_management_json = settings
            .get(AzureAppConfigurationSettingsProvider::FEATURE_MANAGEMENT_KEY)
            .and_then(Option::as_deref)
            .unwrap();
        let feature_management: serde_json::Value =
            serde_json::from_str(feature_management_json).unwrap();
        let feature_flags = &feature_management["feature_management"]["feature_flags"];
        assert_eq!(feature_flags.as_array().unwrap().len(), 1);
        assert_eq!(feature_flags[0]["id"], "beta");
        assert_eq!(feature_flags[0]["enabled"], false);
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
        let configuration_setting_name = String::from("configuration_to_key_vault_secret_2");
        let expected_secret_value = "secret2-value";
        let mut key_vault_reference_loader =
            ParallelAzureKeyVaultReferenceLoader::new(Arc::new(CredentialMock));
        key_vault_reference_loader.with_client_options(SecretClientOptions {
            client_options: ClientOptions {
                transport: Some(Transport::new(Arc::new(KeyVaultHttpClientMock))),
                ..Default::default()
            },
            ..Default::default()
        });
        let configuration_setting = ConfigurationSetting {
            key: configuration_setting_name.clone(),
            content_type: Some(String::from(
                "application/vnd.microsoft.appconfig.keyvaultref+json;charset=utf-8",
            )),
            value: String::from(
                "{\"uri\":\"https://example.vault.azure.net/secrets/Secret2/version1\"}",
            ),
        };
        let secret_reference_uri =
            AzureAppConfigurationSettingsProvider::extract_secret_reference_uri(
                &configuration_setting,
            )
            .unwrap();

        key_vault_reference_loader.add_reference(configuration_setting.key, secret_reference_uri);

        let loaded_values = key_vault_reference_loader.load_all_secrets().await.unwrap();

        assert_eq!(
            loaded_values
                .get(&configuration_setting_name)
                .and_then(|secret| secret.value.as_deref()),
            Some(expected_secret_value)
        );
    }

    #[tokio::test]
    async fn test_load_key_vault_references_from_multiple_vaults() {
        let mut key_vault_reference_loader =
            ParallelAzureKeyVaultReferenceLoader::new(Arc::new(CredentialMock));
        key_vault_reference_loader.with_client_options(SecretClientOptions {
            client_options: ClientOptions {
                transport: Some(Transport::new(Arc::new(KeyVaultHttpClientMock))),
                ..Default::default()
            },
            ..Default::default()
        });
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

    #[tokio::test]
    async fn test_load_key_vault_reference_from_later_selector() {
        Python::initialize();
        let provider = Python::attach(|py| create_provider(py, StatusCode::Ok));
        let mut settings = BTreeMap::new();
        let configuration_settings = vec![
            ConfigurationSetting {
                key: String::from("KeyVault1"),
                content_type: Some(String::from(
                    "application/vnd.microsoft.appconfig.keyvaultref+json;charset=utf-8",
                )),
                value: String::from(
                    "{\"uri\":\"https://example.vault.azure.net/secrets/UnsupportedSecret\"}",
                ),
            },
            ConfigurationSetting {
                key: String::from("KeyVault1"),
                content_type: Some(String::from(
                    "application/vnd.microsoft.appconfig.keyvaultref+json;charset=utf-8",
                )),
                value: String::from(
                    "{\"uri\":\"https://example.vault.azure.net/secrets/Secret2/version1\"}",
                ),
            },
        ];

        provider
            .add_key_vault_reference_configuration_settings(&mut settings, configuration_settings)
            .await
            .unwrap();

        assert_eq!(
            settings.get("KeyVault1").and_then(Option::as_deref),
            Some("secret2-value")
        );
    }

    #[test]
    fn test_leave_configuration_setting_keys_unchanged_when_no_prefixes_are_provided() {
        let configuration_setting_key = String::from("service:application_name");
        let mut configuration_settings = BTreeMap::from([(
            configuration_setting_key.clone(),
            Some(String::from("wirio")),
        )]);

        AzureAppConfigurationSettingsProvider::trim_configuration_setting_key_prefixes(
            &mut configuration_settings,
            &[],
        );

        assert_eq!(
            configuration_settings.get(&configuration_setting_key),
            Some(&Some(String::from("wirio")))
        );
    }

    #[test]
    fn test_leave_feature_flag_names_unchanged_when_no_prefixes_are_provided() {
        let feature_flag_name = String::from("service:beta_feature");
        let mut feature_flags = vec![EnhancedFeatureFlag {
            name: feature_flag_name.clone(),
            enabled: true,
            description: None,
            conditions: None,
            variants: None,
            allocation: None,
            telemetry: None,
        }];

        AzureAppConfigurationSettingsProvider::trim_feature_flag_name_prefixes(
            &mut feature_flags,
            &[],
        );

        assert_eq!(feature_flags[0].name, feature_flag_name);
    }

    #[tokio::test]
    async fn test_resolve_snapshot_references() {
        Python::initialize();
        let provider = Python::attach(|py| create_provider(py, StatusCode::Ok));
        let configurations = vec![
            ConfigurationSetting {
                key: String::from("before"),
                content_type: None,
                value: String::from("first"),
            },
            ConfigurationSetting {
                key: String::from("snapshot"),
                content_type: Some(String::from(
                    "application/json; profile=\"https://azconfig.io/mime-profiles/snapshot-ref\"; charset=utf-8",
                )),
                value: String::from("{\"snapshot_name\":\"production\"}"),
            },
            ConfigurationSetting {
                key: String::from("after"),
                content_type: None,
                value: String::from("last"),
            },
        ];

        let resolved_configurations = provider
            .resolve_snapshot_references(configurations)
            .await
            .unwrap();

        assert_eq!(resolved_configurations.len(), 4);
        assert_eq!(resolved_configurations[0].key, "before");
        assert_eq!(resolved_configurations[1].key, "ApplicationName");
        assert_eq!(resolved_configurations[2].key, "Logging.LogLevel");
        assert_eq!(resolved_configurations[3].key, "after");
    }
}
