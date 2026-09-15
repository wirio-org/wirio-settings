use crate::azure::app_configuration::dtos::{
    Configuration, EnhancedFeatureFlag, GetConfigurationsResponse, GetEnhancedFeatureFlagsResponse,
};
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

    pub(crate) async fn get_configurations(&self) -> azure_core::Result<Vec<Configuration>> {
        let mut url = self.endpoint.clone();
        url.append_path("kv");
        let mut query_builder = url.query_builder();
        query_builder.set_pair("api-version", Self::KEY_VALUES_API_VERSION);
        query_builder.build();
        let mut configurations: Vec<Configuration> = Vec::new();

        loop {
            let response = self.get_configurations_page(&url).await?;
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
    ) -> azure_core::Result<Vec<EnhancedFeatureFlag>> {
        let mut url = self.endpoint.clone();
        url.append_path("ff");
        let mut query_builder = url.query_builder();
        query_builder.set_pair("api-version", Self::ENHANCED_FEATURE_FLAGS_API_VERSION);
        query_builder.build();
        let mut feature_flags = Vec::new();

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

    async fn get_configurations_page(
        &self,
        url: &Url,
    ) -> azure_core::Result<GetConfigurationsResponse> {
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
        Response::<GetConfigurationsResponse>::from(response).into_model()
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

    fn is_feature_flag(configuration: &Configuration) -> bool {
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
                        "https://example.azconfig.io/kv?api-version=2026-04-01"
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
                        "https://example.azconfig.io/ff?api-version=2026-05-01-preview"
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

        let configurations = client.get_configurations().await.unwrap();

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

        let configurations = client.get_configurations().await.unwrap();

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

        let feature_flags = client.get_enhanced_feature_flags().await.unwrap();

        assert_eq!(feature_flags.len(), 2);
        assert_eq!(feature_flags[0].name, "first");
        assert!(feature_flags[0].enabled);
        assert_eq!(feature_flags[1].name, "second");
        assert!(!feature_flags[1].enabled);
    }
}
