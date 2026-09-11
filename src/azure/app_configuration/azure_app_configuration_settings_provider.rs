use crate::{
    azure::app_configuration::azure_app_configuration_client::AzureAppConfigurationClient,
    core::{ModelRegistry, PythonSettingsProvider, SettingLookup, SettingsProvider},
};
use arc_swap::ArcSwap;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::OnceCell;

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct AzureAppConfigurationSettingsProvider {
    data: ArcSwap<Py<PyDict>>,
    endpoint: String,
    client: Arc<AzureAppConfigurationClient>,
    model_registry: OnceCell<Py<ModelRegistry>>,
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
    pub(crate) fn new(
        py: Python<'_>,
        endpoint: String,
        client: Arc<AzureAppConfigurationClient>,
    ) -> Self {
        Self {
            data: ArcSwap::from_pointee(PyDict::new(py).unbind()),
            endpoint,
            client,
            model_registry: OnceCell::new(),
        }
    }
}

impl SettingsProvider for AzureAppConfigurationSettingsProvider {
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        self.data.load().clone_ref(py)
    }

    async fn reload(&self) -> PyResult<()> {
        let configurations = self.client.get_configurations().await.map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to get configurations from Azure App Configuration '{endpoint}': {error}",
                endpoint = self.endpoint
            ))
        })?;
        let mut values = configurations
            .into_iter()
            .map(|configuration| (configuration.key, Some(configuration.value)))
            .collect::<BTreeMap<String, Option<String>>>();
        Self::normalize_keys(&mut values);
        let data: Py<PyDict> = Python::attach(|py| Self::create_data(py, values))?;
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

#[cfg(test)]
mod tests {
    use super::AzureAppConfigurationSettingsProvider;
    use crate::{
        azure::app_configuration::azure_app_configuration_client::AzureAppConfigurationClient,
        core::SettingsProvider,
    };
    use async_trait::async_trait;
    use azure_core::{
        credentials::{AccessToken, TokenCredential, TokenRequestOptions},
        error::ErrorKind,
        http::{
            AsyncRawResponse, ClientOptions, HttpClient, Pipeline, Request, StatusCode, Transport,
            headers::Headers,
        },
    };
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
        response_body: Vec<u8>,
        status_code: StatusCode,
    }

    #[async_trait]
    impl HttpClient for HttpClientMock {
        async fn execute_request(
            &self,
            _request: &Request,
        ) -> azure_core::Result<AsyncRawResponse> {
            Ok(AsyncRawResponse::from_bytes(
                self.status_code,
                Headers::default(),
                self.response_body.clone(),
            ))
        }
    }

    fn create_provider(
        py: Python<'_>,
        response_body: &[u8],
        status_code: StatusCode,
    ) -> AzureAppConfigurationSettingsProvider {
        let client = AzureAppConfigurationClient::new(
            "https://example.azconfig.io",
            Arc::new(CredentialMock),
        )
        .unwrap()
        .with_pipeline(Pipeline::new(
            option_env!("CARGO_PKG_NAME"),
            option_env!("CARGO_PKG_VERSION"),
            ClientOptions {
                transport: Some(Transport::new(Arc::new(HttpClientMock {
                    response_body: response_body.to_vec(),
                    status_code,
                }))),
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
        )
    }

    #[tokio::test]
    async fn test_load_and_normalize_configurations() {
        Python::initialize();
        let provider = Python::attach(|py| {
            create_provider(
                py,
                br#"{"items": [
                    {"key": "ApplicationName", "value": "wirio"},
                    {"key": "Logging.LogLevel", "value": "warning"}
                ]}"#,
                StatusCode::Ok,
            )
        });

        provider.reload().await.unwrap();

        Python::attach(|py| {
            let data = provider.data(py);
            let data = data.bind(py);

            assert_eq!(data.len(), 2);
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
        });
    }

    #[tokio::test]
    async fn test_fail_loading_configurations_when_response_is_invalid() {
        Python::initialize();
        let provider =
            Python::attach(|py| create_provider(py, b"Invalid response", StatusCode::Ok));

        let error = provider.reload().await.unwrap_err();

        assert!(error.to_string().starts_with(
            "RuntimeError: Failed to get configurations from Azure App Configuration 'https://example.azconfig.io':"
        ));
    }

    #[test]
    fn test_display_includes_endpoint() {
        Python::initialize();
        let provider =
            Python::attach(|py| create_provider(py, br#"{"items": []}"#, StatusCode::Ok));

        assert_eq!(
            provider.to_string(),
            "AzureAppConfigurationSettingsProvider {endpoint: https://example.azconfig.io}"
        );
    }
}
