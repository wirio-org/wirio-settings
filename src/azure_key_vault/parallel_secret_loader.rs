use azure_security_keyvault_secrets::SecretClient;
use azure_security_keyvault_secrets::models::Secret;
use futures::{StreamExt, TryStreamExt};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::collections::BTreeMap;

pub(crate) struct ParallelSecretLoader<'a> {
    secret_client: &'a SecretClient,
    secret_names: Vec<String>,
}

impl<'a> ParallelSecretLoader<'a> {
    const PARALLELISM_LEVEL: usize = 32;

    pub(crate) fn new(secret_client: &'a SecretClient) -> Self {
        Self {
            secret_client,
            secret_names: Vec::new(),
        }
    }

    pub(crate) fn add_secret_to_load(&mut self, secret_name: String) {
        self.secret_names.push(secret_name);
    }

    pub(crate) async fn load_all_secrets(self, url: &str) -> PyResult<BTreeMap<String, Secret>> {
        let mut loaded_secrets = futures::stream::iter(
            self.secret_names
                .into_iter()
                .map(|secret_name| Self::retrieve_secret(self.secret_client, secret_name, url)),
        )
        .buffer_unordered(Self::PARALLELISM_LEVEL);
        let mut new_loaded_secrets = BTreeMap::new();

        while let Some((secret_name, secret)) = loaded_secrets.try_next().await? {
            new_loaded_secrets.insert(secret_name, secret);
        }

        Ok(new_loaded_secrets)
    }

    async fn retrieve_secret(
        secret_client: &SecretClient,
        secret_name: String,
        url: &str,
    ) -> PyResult<(String, Secret)> {
        let secret_response = secret_client.get_secret(&secret_name, None).await.map_err(
            |error| {
                PyRuntimeError::new_err(format!(
                    "Failed to read secret '{secret_name}' from Azure Key Vault '{url}': {error}",
                ))
            },
        )?;

        let secret = secret_response.into_model().map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to deserialize Azure Key Vault secret '{secret_name}': {error}",
            ))
        })?;

        Ok((secret_name, secret))
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.secret_names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::ParallelSecretLoader;
    use async_trait::async_trait;
    use azure_core::credentials::{AccessToken, TokenCredential, TokenRequestOptions};
    use azure_core::http::{
        AsyncRawResponse, ClientOptions, HttpClient, Request, StatusCode, Transport,
        headers::Headers,
    };
    use azure_security_keyvault_secrets::{SecretClient, SecretClientOptions};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
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
        active_requests: Arc<AtomicUsize>,
        maximum_active_requests: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl HttpClient for HttpClientMock {
        async fn execute_request(
            &self,
            _request: &Request,
        ) -> azure_core::Result<AsyncRawResponse> {
            let active_requests = self.active_requests.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum_active_requests
                .fetch_max(active_requests, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(200)).await;
            self.active_requests.fetch_sub(1, Ordering::SeqCst);

            Ok(AsyncRawResponse::from_bytes(
                StatusCode::Ok,
                Headers::default(),
                br#"{"value":"retrieved-value"}"#.to_vec(),
            ))
        }
    }

    #[test]
    fn test_create_loader_with_no_secret_names() {
        let secret_client = SecretClient::new(
            "https://example.vault.azure.net",
            Arc::new(CredentialMock),
            None,
        )
        .unwrap();

        let parallel_secret_loader = ParallelSecretLoader::new(&secret_client);

        assert!(parallel_secret_loader.is_empty());
    }

    #[tokio::test]
    async fn test_load_queued_secrets_in_parallel() {
        let expected_loaded_secrets_count = 2;
        let expected_first_secret_name = "first-secret";
        let expected_second_secret_name = "second-secret";
        let expected_secret_value = "retrieved-value";
        let active_requests = Arc::new(AtomicUsize::new(0));
        let maximum_active_requests = Arc::new(AtomicUsize::new(0));
        let url = "https://example.vault.azure.net";
        let secret_client_options = SecretClientOptions {
            client_options: ClientOptions {
                transport: Some(Transport::new(Arc::new(HttpClientMock {
                    active_requests: Arc::clone(&active_requests),
                    maximum_active_requests: Arc::clone(&maximum_active_requests),
                }))),
                ..Default::default()
            },
            ..Default::default()
        };
        let secret_client =
            SecretClient::new(url, Arc::new(CredentialMock), Some(secret_client_options)).unwrap();
        let mut parallel_secret_loader = ParallelSecretLoader::new(&secret_client);
        parallel_secret_loader.add_secret_to_load(String::from(expected_first_secret_name));
        parallel_secret_loader.add_secret_to_load(String::from(expected_second_secret_name));

        let loaded_secrets_from_loader =
            parallel_secret_loader.load_all_secrets(url).await.unwrap();

        assert_eq!(
            loaded_secrets_from_loader.len(),
            expected_loaded_secrets_count
        );
        assert_eq!(
            loaded_secrets_from_loader
                .get(expected_first_secret_name)
                .and_then(|secret| secret.value.as_deref()),
            Some(expected_secret_value)
        );
        assert_eq!(
            loaded_secrets_from_loader
                .get(expected_second_secret_name)
                .and_then(|secret| secret.value.as_deref()),
            Some(expected_secret_value)
        );
        assert_eq!(maximum_active_requests.load(Ordering::SeqCst), 2);
    }
}
