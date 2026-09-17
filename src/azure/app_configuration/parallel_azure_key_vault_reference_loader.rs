use crate::azure::core::remove_user_agent::RemoveUserAgent;
use azure_core::{credentials::TokenCredential, http::Url};
use azure_security_keyvault_secrets::{
    SecretClient, SecretClientOptions,
    models::{Secret, SecretClientGetSecretOptions},
};
use futures::{StreamExt, TryStreamExt};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) struct ParallelAzureKeyVaultReferenceLoader {
    credential: Arc<dyn TokenCredential>,
    client_options: SecretClientOptions,
    references: Vec<AzureKeyVaultReference>,
}

impl ParallelAzureKeyVaultReferenceLoader {
    const PARALLELISM_LEVEL: usize = 32;

    pub(crate) fn new(credential: Arc<dyn TokenCredential>) -> Self {
        Self {
            credential,
            client_options: SecretClientOptions::default(),
            references: Vec::new(),
        }
    }

    pub(crate) fn add_reference(&mut self, configuration_key: String, secret_reference_uri: Url) {
        self.references.push(AzureKeyVaultReference {
            configuration_key,
            secret_reference_uri,
        });
    }

    pub(crate) async fn load_all_secrets(self) -> PyResult<BTreeMap<String, Secret>> {
        let references_by_vault = self.get_references_by_vault()?;
        let secret_loaders = Self::create_secret_loaders(
            references_by_vault,
            &self.credential,
            &self.client_options,
        )?;
        let secret_retrievals = secret_loaders.into_iter().flat_map(|secret_loader| {
            secret_loader.references.into_iter().map(move |reference| {
                Self::retrieve_secret(Arc::clone(&secret_loader.client), reference)
            })
        });
        let mut loaded_secrets =
            futures::stream::iter(secret_retrievals).buffer_unordered(Self::PARALLELISM_LEVEL);
        let mut new_loaded_secrets = BTreeMap::new();

        while let Some(retrieved_secret) = loaded_secrets.try_next().await? {
            new_loaded_secrets.insert(retrieved_secret.name, retrieved_secret.secret);
        }

        Ok(new_loaded_secrets)
    }

    fn create_secret_loaders(
        references_by_vault: BTreeMap<String, AzureKeyVaultReferencesByVault>,
        credential: &Arc<dyn TokenCredential>,
        client_options: &SecretClientOptions,
    ) -> PyResult<Vec<AzureKeyVaultSecretLoader>> {
        references_by_vault
            .into_values()
            .map(|vault_references_by_vault| {
                Self::create_secret_loader(
                    vault_references_by_vault,
                    Arc::clone(credential),
                    client_options,
                )
            })
            .collect()
    }

    fn create_secret_loader(
        vault_references_by_vault: AzureKeyVaultReferencesByVault,
        credential: Arc<dyn TokenCredential>,
        client_options: &SecretClientOptions,
    ) -> PyResult<AzureKeyVaultSecretLoader> {
        let client = Arc::new(Self::create_secret_client(
            &vault_references_by_vault.vault_uri,
            credential,
            client_options.clone(),
        )?);

        Ok(AzureKeyVaultSecretLoader {
            client,
            references: vault_references_by_vault.references,
        })
    }

    fn get_references_by_vault(
        &self,
    ) -> PyResult<BTreeMap<String, AzureKeyVaultReferencesByVault>> {
        let mut references_by_vault = BTreeMap::new();

        for reference in &self.references {
            let secret_information =
                Self::extract_secret_information(&reference.secret_reference_uri)?;

            references_by_vault
                .entry(secret_information.vault_uri.to_string())
                .or_insert_with(|| AzureKeyVaultReferencesByVault {
                    vault_uri: secret_information.vault_uri,
                    references: Vec::new(),
                })
                .references
                .push(AzureKeyVaultReferencesByVaultReference {
                    configuration_key: reference.configuration_key.clone(),
                    secret_name: secret_information.secret_name,
                    secret_version: secret_information.secret_version,
                    secret_reference_uri: reference.secret_reference_uri.clone(),
                });
        }

        Ok(references_by_vault)
    }

    fn create_secret_client(
        vault_uri: &Url,
        credential: Arc<dyn TokenCredential>,
        mut client_options: SecretClientOptions,
    ) -> PyResult<SecretClient> {
        client_options
            .client_options
            .per_call_policies
            .push(Arc::new(RemoveUserAgent));
        SecretClient::new(vault_uri.as_str(), credential, Some(client_options)).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to create Azure Key Vault client for '{vault_uri}': {error}",
            ))
        })
    }

    async fn retrieve_secret(
        secret_client: Arc<SecretClient>,
        secret_reference: AzureKeyVaultReferencesByVaultReference,
    ) -> PyResult<RetrievedSecret> {
        let secret_response = secret_client
            .get_secret(
                &secret_reference.secret_name,
                secret_reference.secret_version.map(|secret_version| SecretClientGetSecretOptions {
                    secret_version: Some(secret_version),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to read Azure Key Vault reference '{}' for Azure App Configuration key '{}': {error}",
                    secret_reference.secret_reference_uri, secret_reference.configuration_key,
                ))
            })?;

        let secret = secret_response.into_model().map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to deserialize Azure Key Vault secret for Azure App Configuration key '{}': {error}",
                secret_reference.configuration_key,
            ))
        })?;

        Ok(RetrievedSecret {
            name: secret_reference.configuration_key,
            secret,
        })
    }

    fn extract_secret_information(secret_reference_uri: &Url) -> PyResult<SecretInformation> {
        let mut path_segments = secret_reference_uri.path_segments().ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "Invalid Azure Key Vault reference URI '{secret_reference_uri}': missing path segments",
            ))
        })?;
        let resource_type = path_segments.next();
        let secret_name = path_segments.next();
        let secret_version = path_segments.next();
        let has_additional_path_segments = path_segments.next().is_some();

        if resource_type != Some("secrets") || secret_name.is_none() || has_additional_path_segments
        {
            return Err(PyRuntimeError::new_err(format!(
                "Invalid Azure Key Vault reference URI '{secret_reference_uri}': expected '/secrets/<name>[/<version>]'",
            )));
        }

        let mut vault_uri = secret_reference_uri.clone();
        vault_uri.set_path("");
        vault_uri.set_query(None);
        vault_uri.set_fragment(None);

        if vault_uri.host_str().is_none() {
            return Err(PyRuntimeError::new_err(format!(
                "Invalid Azure Key Vault reference URI '{secret_reference_uri}': missing vault host",
            )));
        }

        Ok(SecretInformation {
            vault_uri,
            secret_name: String::from(secret_name.unwrap_or_default()),
            secret_version: secret_version.map(String::from),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_client_options(
        credential: Arc<dyn TokenCredential>,
        client_options: SecretClientOptions,
    ) -> Self {
        let mut loader = Self::new(credential);
        loader.client_options = client_options;
        loader
    }
}

struct AzureKeyVaultReference {
    configuration_key: String,
    secret_reference_uri: Url,
}

struct RetrievedSecret {
    name: String,
    secret: Secret,
}

struct SecretInformation {
    vault_uri: Url,
    secret_name: String,
    secret_version: Option<String>,
}

struct AzureKeyVaultReferencesByVault {
    vault_uri: Url,
    references: Vec<AzureKeyVaultReferencesByVaultReference>,
}

struct AzureKeyVaultSecretLoader {
    client: Arc<SecretClient>,
    references: Vec<AzureKeyVaultReferencesByVaultReference>,
}

struct AzureKeyVaultReferencesByVaultReference {
    configuration_key: String,
    secret_name: String,
    secret_version: Option<String>,
    secret_reference_uri: Url,
}

#[cfg(test)]
mod tests {
    use super::ParallelAzureKeyVaultReferenceLoader;
    use azure_core::http::Url;
    use pyo3::*;

    #[test]
    fn test_extract_unversioned_secret_information() {
        let secret_information = ParallelAzureKeyVaultReferenceLoader::extract_secret_information(
            &Url::parse("https://example.vault.azure.net/secrets/Secret1").unwrap(),
        )
        .unwrap();

        assert_eq!(
            secret_information.vault_uri.as_str(),
            "https://example.vault.azure.net/"
        );
        assert_eq!(secret_information.secret_name, "Secret1");
        assert_eq!(secret_information.secret_version, None);
    }

    #[test]
    fn test_extract_versioned_secret_information() {
        let secret_information = ParallelAzureKeyVaultReferenceLoader::extract_secret_information(
            &Url::parse("https://example.vault.azure.net/secrets/Secret2/version1").unwrap(),
        )
        .unwrap();

        assert_eq!(
            secret_information.vault_uri.as_str(),
            "https://example.vault.azure.net/"
        );
        assert_eq!(secret_information.secret_name, "Secret2");
        assert_eq!(
            secret_information.secret_version,
            Some(String::from("version1"))
        );
    }

    #[test]
    fn test_fail_extracting_secret_information_with_invalid_path() {
        let error = ParallelAzureKeyVaultReferenceLoader::extract_secret_information(
            &Url::parse("https://example.vault.azure.net/keys/Secret1").unwrap(),
        )
        .err()
        .unwrap();

        assert_eq!(
            error.to_string(),
            "RuntimeError: Invalid Azure Key Vault reference URI 'https://example.vault.azure.net/keys/Secret1': expected '/secrets/<name>[/<version>]'"
        );
    }

    #[test]
    fn test_fail_extracting_secret_information_without_vault_host() {
        Python::initialize();

        let error = ParallelAzureKeyVaultReferenceLoader::extract_secret_information(
            &Url::parse("file:///secrets/Secret1").unwrap(),
        )
        .err()
        .unwrap();

        assert_eq!(
            error.to_string(),
            "RuntimeError: Invalid Azure Key Vault reference URI 'file:///secrets/Secret1': missing vault host"
        );
    }
}
