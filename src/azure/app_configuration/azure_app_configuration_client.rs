use crate::azure::app_configuration::dtos::{
    ConfigurationSetting, EnhancedFeatureFlag, GetConfigurationSettingsResponse,
    GetEnhancedFeatureFlagsResponse,
};
use crate::azure::app_configuration::models::{FeatureFlagSelector, SettingSelector};
use azure_core::{
    credentials::TokenCredential,
    error::CheckSuccessOptions,
    http::{
        ClientOptions, Context, Method, Pipeline, PipelineSendOptions, Request, Response, Url,
        UrlExt,
        policies::{Policy, auth::BearerTokenAuthorizationPolicy},
    },
};
use std::sync::Arc;

pub(crate) struct AzureAppConfigurationClient {
    endpoint: Url,
    pipeline: Pipeline,
}

impl AzureAppConfigurationClient {
    // https://learn.microsoft.com/en-us/azure/azure-app-configuration/rest-api-key-value?pivots=v26-04
    const KEY_VALUES_API_VERSION: &str = "2026-04-01";

    // https://learn.microsoft.com/en-us/azure/azure-app-configuration/rest-api-enhanced-feature-flag?pivots=v26-05-preview
    const ENHANCED_FEATURE_FLAGS_API_VERSION: &str = "2026-05-01-preview";

    const APP_CONFIGURATION_SCOPE: &str = "https://azconfig.io/.default";
    const FEATURE_FLAG_KEY_PREFIX: &str = ".appconfig.featureflag/";

    pub(crate) fn new(
        endpoint: &str,
        credential: Arc<dyn TokenCredential>,
    ) -> azure_core::Result<Self> {
        let endpoint = Url::parse(endpoint)?;
        let auth_policy: Arc<dyn Policy> = Arc::new(BearerTokenAuthorizationPolicy::new(
            credential,
            vec![String::from(Self::APP_CONFIGURATION_SCOPE)],
        ));

        Ok(Self {
            endpoint,
            pipeline: Pipeline::new(
                option_env!("CARGO_PKG_NAME"),
                option_env!("CARGO_PKG_VERSION"),
                ClientOptions::default(),
                Vec::new(),
                vec![auth_policy],
                None,
            ),
        })
    }

    pub(crate) async fn get_configuration_settings(
        &self,
        selector: &SettingSelector,
    ) -> azure_core::Result<Vec<ConfigurationSetting>> {
        let mut configurations: Vec<ConfigurationSetting> = Vec::new();
        let mut url = self
            .get_url_for_getting_configuration_settings(selector)
            .await?;

        loop {
            let response = self.get_configuration_settings_page(&url).await?;
            let configurations_to_add = response
                .items
                .into_iter()
                .filter(|configuration| !Self::is_feature_flag(configuration));
            configurations.extend(configurations_to_add);

            let Some(next_link) = response.next_link else {
                break;
            };

            url = self.endpoint.join(&next_link)?;
        }

        Ok(configurations)
    }

    pub(crate) async fn get_enhanced_feature_flags(
        &self,
        selector: &FeatureFlagSelector,
    ) -> azure_core::Result<Vec<EnhancedFeatureFlag>> {
        let mut feature_flags = Vec::new();
        let mut url = self.get_url_for_getting_enhanced_feature_flags(selector);

        loop {
            let response = self.get_feature_flags_page(&url).await?;
            feature_flags.extend(response.items);

            let Some(next_link) = response.next_link else {
                break;
            };

            url = self.endpoint.join(&next_link)?;
        }

        Ok(feature_flags)
    }

    async fn get_url_for_getting_configuration_settings(
        &self,
        selector: &SettingSelector,
    ) -> azure_core::Result<Url> {
        if let Some(snapshot_name) = &selector.snapshot_name {
            self.ensure_configuration_snapshot_exists(snapshot_name)
                .await?;
            return Ok(self.get_url_for_getting_configuration_snapshot_settings(snapshot_name));
        }

        Ok(self.get_url_for_getting_key_value_configuration_settings(selector))
    }

    fn get_url_for_getting_key_value_configuration_settings(
        &self,
        selector: &SettingSelector,
    ) -> Url {
        let mut url = self.endpoint.clone();
        url.append_path("kv");
        let mut query_builder = url.query_builder();
        query_builder.set_pair("api-version", Self::KEY_VALUES_API_VERSION);

        if let Some(key_filter) = &selector.key_filter {
            query_builder.set_pair("key", key_filter);
        }

        if let Some(label_filter) = &selector.label_filter {
            query_builder.set_pair("label", label_filter);
        }

        query_builder.build();
        url
    }

    fn get_url_for_getting_configuration_snapshot(&self, snapshot_name: &str) -> Url {
        let mut url = self.endpoint.clone();
        url.append_path("snapshots");
        url.append_path(snapshot_name);
        let mut query_builder = url.query_builder();
        query_builder.set_pair("api-version", Self::KEY_VALUES_API_VERSION);
        query_builder.build();
        url
    }

    fn get_url_for_getting_configuration_snapshot_settings(&self, snapshot_name: &str) -> Url {
        let mut url = self.endpoint.clone();
        url.append_path("kv");
        let mut query_builder = url.query_builder();
        query_builder.set_pair("api-version", Self::KEY_VALUES_API_VERSION);
        query_builder.set_pair("snapshot", snapshot_name);
        query_builder.build();
        url
    }

    async fn ensure_configuration_snapshot_exists(
        &self,
        snapshot_name: &str,
    ) -> azure_core::Result<()> {
        let url = self.get_url_for_getting_configuration_snapshot(snapshot_name);
        let mut request = Request::new(url, Method::Get);
        request.insert_header(
            "accept",
            "application/vnd.microsoft.appconfig.snapshot+json; charset=utf-8",
        );
        self.pipeline
            .send(
                &Context::default(),
                &mut request,
                Some(PipelineSendOptions {
                    check_success: CheckSuccessOptions {
                        success_codes: &[200],
                    },
                    ..Default::default()
                }),
            )
            .await?;
        Ok(())
    }

    fn get_url_for_getting_enhanced_feature_flags(&self, selector: &FeatureFlagSelector) -> Url {
        let mut url = self.endpoint.clone();
        url.append_path("ff");
        let mut query_builder = url.query_builder();
        query_builder.set_pair("api-version", Self::ENHANCED_FEATURE_FLAGS_API_VERSION);
        query_builder.set_pair("name", &selector.name_filter);
        query_builder.set_pair("label", &selector.label_filter);
        query_builder.build();
        url
    }

    async fn get_configuration_settings_page(
        &self,
        url: &Url,
    ) -> azure_core::Result<GetConfigurationSettingsResponse> {
        let mut request = Request::new(url.clone(), Method::Get);
        request.insert_header(
            "accept",
            "application/vnd.microsoft.appconfig.kvset+json; charset=utf-8",
        );
        let response = self
            .pipeline
            .send(
                &Context::default(),
                &mut request,
                Some(PipelineSendOptions {
                    check_success: CheckSuccessOptions {
                        success_codes: &[200],
                    },
                    ..Default::default()
                }),
            )
            .await?;
        Response::<GetConfigurationSettingsResponse>::from(response).into_model()
    }

    async fn get_feature_flags_page(
        &self,
        url: &Url,
    ) -> azure_core::Result<GetEnhancedFeatureFlagsResponse> {
        let mut request = Request::new(url.clone(), Method::Get);
        request.insert_header(
            "accept",
            r#"application/json; profile="https://azconfig.io/mime-profiles/ffset"; charset=utf-8"#,
        );
        let response = self
            .pipeline
            .send(
                &Context::default(),
                &mut request,
                Some(PipelineSendOptions {
                    check_success: CheckSuccessOptions {
                        success_codes: &[200],
                    },
                    ..Default::default()
                }),
            )
            .await?;
        Response::<GetEnhancedFeatureFlagsResponse>::from(response).into_model()
    }

    fn is_feature_flag(configuration: &ConfigurationSetting) -> bool {
        configuration.key.starts_with(Self::FEATURE_FLAG_KEY_PREFIX)
    }

    #[cfg(test)]
    pub(crate) fn with_pipeline(mut self, pipeline: Pipeline) -> Self {
        self.pipeline = pipeline;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::AzureAppConfigurationClient;
    use crate::azure::app_configuration::{FeatureFlagSelector, SettingSelector};
    use async_trait::async_trait;
    use azure_core::{
        credentials::{AccessToken, TokenCredential, TokenRequestOptions},
        http::{
            AsyncRawResponse, ClientOptions, HttpClient, Pipeline, Request, StatusCode, Transport,
            headers::Headers,
        },
    };
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

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
    struct HttpClientMock;

    #[async_trait]
    impl HttpClient for HttpClientMock {
        async fn execute_request(&self, request: &Request) -> azure_core::Result<AsyncRawResponse> {
            let response_body = match request.url().path() {
                "/kv" => {
                    assert_eq!(
                        request.url().as_str(),
                        "https://example.azconfig.io/kv?api-version=2026-04-01&key=*&label=%00"
                    );
                    br#"{
						"items": [
                            {"key": "service_name", "value": "wirio-api"},
                            {"key": "app.name", "value": "wirio"},
                            {"key": ".appconfig.featureflag/beta", "value": "{}"}
						]
                    }"#
                    .as_slice()
                }
                "/ff" => {
                    assert_eq!(
                        request.url().as_str(),
                        "https://example.azconfig.io/ff?api-version=2026-05-01-preview&name=*&label=%00"
                    );
                    br#"{"items": [{"name": "beta", "enabled": true}]}"#.as_slice()
                }
                path => panic!("Unexpected request path: {path}"),
            };

            Ok(AsyncRawResponse::from_bytes(
                StatusCode::Ok,
                Headers::default(),
                response_body,
            ))
        }
    }

    #[derive(Debug)]
    struct SelectorHttpClientMock;

    #[async_trait]
    impl HttpClient for SelectorHttpClientMock {
        async fn execute_request(&self, request: &Request) -> azure_core::Result<AsyncRawResponse> {
            assert_eq!(request.url().path(), "/kv");
            assert!(
                request
                    .url()
                    .query_pairs()
                    .any(|(key, value)| key == "key" && value == "service.*")
            );
            assert!(
                request
                    .url()
                    .query_pairs()
                    .any(|(key, value)| key == "label" && value == "\0")
            );

            Ok(AsyncRawResponse::from_bytes(
                StatusCode::Ok,
                Headers::default(),
                br#"{"items": [{"key": "service.name", "value": "wirio"}]}"#.as_slice(),
            ))
        }
    }

    #[derive(Debug)]
    struct EnhancedFeatureFlagSelectorHttpClientMock;

    #[async_trait]
    impl HttpClient for EnhancedFeatureFlagSelectorHttpClientMock {
        async fn execute_request(&self, request: &Request) -> azure_core::Result<AsyncRawResponse> {
            assert_eq!(request.url().path(), "/ff");
            assert!(
                request
                    .url()
                    .query_pairs()
                    .any(|(key, value)| key == "name" && value == "beta,gamma")
            );
            assert!(
                request
                    .url()
                    .query_pairs()
                    .any(|(key, value)| key == "label" && value == "\0")
            );

            Ok(AsyncRawResponse::from_bytes(
                StatusCode::Ok,
                Headers::default(),
                br#"{"items": [{"name": "beta", "enabled": true}]}"#.as_slice(),
            ))
        }
    }

    #[derive(Debug)]
    struct PaginatedHttpClientMock {
        responses: Mutex<VecDeque<Vec<u8>>>,
    }

    #[async_trait]
    impl HttpClient for PaginatedHttpClientMock {
        async fn execute_request(
            &self,
            _request: &Request,
        ) -> azure_core::Result<AsyncRawResponse> {
            let response_body = self.responses.lock().unwrap().pop_front().unwrap();

            Ok(AsyncRawResponse::from_bytes(
                StatusCode::Ok,
                Headers::default(),
                response_body,
            ))
        }
    }

    fn create_client(http_client_mock: Arc<dyn HttpClient>) -> AzureAppConfigurationClient {
        AzureAppConfigurationClient::new("https://example.azconfig.io", Arc::new(CredentialMock))
            .unwrap()
            .with_pipeline(Pipeline::new(
                option_env!("CARGO_PKG_NAME"),
                option_env!("CARGO_PKG_VERSION"),
                ClientOptions {
                    transport: Some(Transport::new(http_client_mock)),
                    ..Default::default()
                },
                Vec::new(),
                Vec::new(),
                None,
            ))
    }

    #[tokio::test]
    async fn test_get_configurations_and_exclude_feature_flags() {
        let client = create_client(Arc::new(HttpClientMock));
        let selector = SettingSelector::new(Some(String::from("*")), None, None).unwrap();

        let configurations = client.get_configuration_settings(&selector).await.unwrap();

        assert_eq!(configurations.len(), 2);
        assert_eq!(configurations[0].key, "service_name");
        assert_eq!(configurations[0].value, "wirio-api");
        assert_eq!(configurations[1].key, "app.name");
        assert_eq!(configurations[1].value, "wirio");
    }

    #[tokio::test]
    async fn test_get_configurations_and_follow_next_link() {
        let first_page = br#"{
            "items": [{"key": "first", "value": "one"}],
            "@nextLink": "/kv?api-version=2026-04-01&after=next-reference"
        }"#
        .to_vec();
        let second_page = br#"{"items": [{"key": "second", "value": "two"}]}"#.to_vec();
        let client = create_client(Arc::new(PaginatedHttpClientMock {
            responses: Mutex::new(VecDeque::from([first_page, second_page])),
        }));
        let selector = SettingSelector::new(Some(String::from("*")), None, None).unwrap();

        let configurations = client.get_configuration_settings(&selector).await.unwrap();

        assert_eq!(configurations.len(), 2);
        assert_eq!(configurations[0].key, "first");
        assert_eq!(configurations[1].key, "second");
    }

    #[tokio::test]
    async fn test_get_enhanced_feature_flags_and_follow_next_link() {
        let first_page = br#"{
            "items": [{"name": "first", "enabled": true}],
            "@nextLink": "/ff?api-version=2026-05-01-preview&after=next-reference"
        }"#
        .to_vec();
        let second_page = br#"{"items": [{"name": "second", "enabled": false}]}"#.to_vec();
        let client = create_client(Arc::new(PaginatedHttpClientMock {
            responses: Mutex::new(VecDeque::from([first_page, second_page])),
        }));
        let selector = FeatureFlagSelector::new(String::from("*"), None);

        let feature_flags = client.get_enhanced_feature_flags(&selector).await.unwrap();

        assert_eq!(feature_flags.len(), 2);
        assert_eq!(feature_flags[0].name, "first");
        assert!(feature_flags[0].enabled);
        assert_eq!(feature_flags[1].name, "second");
        assert!(!feature_flags[1].enabled);
    }

    #[tokio::test]
    async fn test_filter_configurations_using_selector() {
        let selectors =
            [SettingSelector::new(Some(String::from("service.*")), None, None).unwrap()];
        let client = create_client(Arc::new(SelectorHttpClientMock));

        let configurations = client
            .get_configuration_settings(&selectors[0])
            .await
            .unwrap();

        assert_eq!(configurations.len(), 1);
        assert_eq!(configurations[0].key, "service.name");
    }

    #[tokio::test]
    async fn test_filter_enhanced_feature_flags_using_feature_flag_selector() {
        let client = create_client(Arc::new(EnhancedFeatureFlagSelectorHttpClientMock));
        let selector = FeatureFlagSelector::new(String::from("beta,gamma"), None);

        let feature_flags = client.get_enhanced_feature_flags(&selector).await.unwrap();

        assert_eq!(feature_flags.len(), 1);
        assert_eq!(feature_flags[0].name, "beta");
    }

    #[tokio::test]
    async fn test_get_configurations_using_snapshot_selector() {
        #[derive(Debug)]
        struct SnapshotHttpClientMock;

        #[async_trait]
        impl HttpClient for SnapshotHttpClientMock {
            async fn execute_request(
                &self,
                request: &Request,
            ) -> azure_core::Result<AsyncRawResponse> {
                let response_body = match request.url().path() {
                    "/snapshots/payment-2026-10-15" => {
                        assert_eq!(
                            request.url().as_str(),
                            "https://example.azconfig.io/snapshots/payment-2026-10-15?api-version=2026-04-01"
                        );
                        br#"{"name": "payment-2026-10-15"}"#.as_slice()
                    }
                    "/kv" => {
                        assert_eq!(
                            request.url().as_str(),
                            "https://example.azconfig.io/kv?api-version=2026-04-01&snapshot=payment-2026-10-15"
                        );
                        br#"{"items": [{"key": "service.name", "value": "wirio"}]}"#.as_slice()
                    }
                    path => panic!("Unexpected request path: {path}"),
                };

                Ok(AsyncRawResponse::from_bytes(
                    StatusCode::Ok,
                    Headers::default(),
                    response_body,
                ))
            }
        }

        let client = create_client(Arc::new(SnapshotHttpClientMock));
        let selector =
            SettingSelector::new(None, None, Some(String::from("payment-2026-10-15"))).unwrap();

        let configurations = client.get_configuration_settings(&selector).await.unwrap();

        assert_eq!(configurations.len(), 1);
        assert_eq!(configurations[0].key, "service.name");
    }
}
