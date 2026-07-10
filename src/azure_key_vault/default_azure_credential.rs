use azure_core::credentials::{AccessToken, TokenCredential, TokenRequestOptions};
use azure_core::error::ErrorKind;
use azure_identity::{
    ClientSecretCredential, DeveloperToolsCredential, ManagedIdentityCredential,
    ManagedIdentityCredentialOptions, UserAssignedId, WorkloadIdentityCredential,
};
use std::env;
use std::fmt;
use std::sync::Arc;

type CredentialSourceFactory = fn() -> azure_core::Result<Arc<dyn TokenCredential>>;

/// Simplifies authentication while developing apps that deploy to Azure by combining credentials used in Azure hosting environments with credentials used in local development.
//
/// Attempts to authenticate with each of these credentials, in the following order, stopping when one provides a token:
/// - `EnvironmentCredential`
/// - `WorkloadIdentityCredential`
/// - `ManagedIdentityCredential`
/// - `AzureCliCredential`
/// - `AzureDeveloperCliCredential`
pub struct DefaultAzureCredential {
    sources_factories: Vec<CredentialSourceFactory>,
}

impl DefaultAzureCredential {
    pub fn new() -> Arc<Self> {
        let source_factories: Vec<CredentialSourceFactory> = vec![
            Self::create_environment_credential,
            Self::create_workload_identity_credential,
            Self::create_managed_identity_credential,
            Self::create_developer_tools_credential,
        ];
        Arc::new(Self {
            sources_factories: source_factories,
        })
    }

    fn create_environment_credential() -> azure_core::Result<Arc<dyn TokenCredential>> {
        let tenant_id = env::var_os("AZURE_TENANT_ID")
            .and_then(|value| value.into_string().ok())
            .ok_or_else(|| {
                azure_core::Error::with_message(
                    ErrorKind::Credential,
                    "Missing 'AZURE_TENANT_ID' for environment credential",
                )
            })?;
        let client_id = env::var_os("AZURE_CLIENT_ID")
            .and_then(|value| value.into_string().ok())
            .ok_or_else(|| {
                azure_core::Error::with_message(
                    ErrorKind::Credential,
                    "Missing 'AZURE_CLIENT_ID' for environment credential",
                )
            })?;
        let client_secret = env::var_os("AZURE_CLIENT_SECRET")
            .and_then(|value| value.into_string().ok())
            .ok_or_else(|| {
                azure_core::Error::with_message(
                    ErrorKind::Credential,
                    "Missing 'AZURE_CLIENT_SECRET' for environment credential",
                )
            })?;
        let credential =
            ClientSecretCredential::new(&tenant_id, client_id, client_secret.into(), None)?;
        Ok(credential)
    }

    fn create_managed_identity_credential() -> azure_core::Result<Arc<dyn TokenCredential>> {
        let user_assigned_client_id =
            env::var_os("AZURE_CLIENT_ID").and_then(|value| value.into_string().ok());
        let options = user_assigned_client_id.map(|client_id| ManagedIdentityCredentialOptions {
            user_assigned_id: Some(UserAssignedId::ClientId(client_id)),
            ..Default::default()
        });
        ManagedIdentityCredential::new(options)
            .map(|credential| credential as Arc<dyn TokenCredential>)
    }

    fn create_workload_identity_credential() -> azure_core::Result<Arc<dyn TokenCredential>> {
        WorkloadIdentityCredential::new(None)
            .map(|credential| credential as Arc<dyn TokenCredential>)
    }

    fn create_developer_tools_credential() -> azure_core::Result<Arc<dyn TokenCredential>> {
        DeveloperToolsCredential::new(None).map(|credential| credential as Arc<dyn TokenCredential>)
    }

    fn format_errors(errors: &[azure_core::Error]) -> String {
        use std::error::Error;
        errors
            .iter()
            .map(|error| {
                let mut current: Option<&dyn Error> = Some(error);
                let mut stack = vec![];

                while let Some(error) = current.take() {
                    stack.push(error.to_string());
                    current = error.source();
                }

                stack.join(" - ")
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

impl fmt::Debug for DefaultAzureCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DefaultAzureCredential")
    }
}

#[async_trait::async_trait]
impl TokenCredential for DefaultAzureCredential {
    async fn get_token(
        &self,
        scopes: &[&str],
        options: Option<TokenRequestOptions<'_>>,
    ) -> azure_core::Result<AccessToken> {
        let mut errors = Vec::new();

        for source_factory in &self.sources_factories {
            match source_factory() {
                Ok(source) => match source.get_token(scopes, options.clone()).await {
                    Ok(token) => return Ok(token),
                    Err(error) => errors.push(error),
                },
                Err(error) => errors.push(error),
            }
        }

        Err(azure_core::Error::with_message_fn(
            ErrorKind::Credential,
            || {
                format!(
                    "Multiple errors were encountered while attempting to authenticate:\n{}",
                    DefaultAzureCredential::format_errors(&errors)
                )
            },
        ))
    }
}
