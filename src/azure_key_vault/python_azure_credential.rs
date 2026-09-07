use crate::azure_key_vault::default_azure_credential::DefaultAzureCredential;
use azure_core::credentials::TokenCredential;
use azure_identity::{
    AzureCliCredential, AzureDeveloperCliCredential, ClientSecretCredential,
    ManagedIdentityCredential, WorkloadIdentityCredential,
};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "AzureCredential", frozen)]
pub enum PythonAzureCredential {
    Default(),
    ManagedIdentityCredential(),
    WorkloadIdentityCredential(),
    AzureCli(),
    AzureDeveloperCli(),
    ClientSecret {
        tenant_id: String,
        client_id: String,
        client_secret: String,
    },
}

impl PythonAzureCredential {
    pub(crate) fn to_token_credential(&self) -> PyResult<Arc<dyn TokenCredential>> {
        let credential: Arc<dyn TokenCredential> = match self {
            Self::Default() => Ok(DefaultAzureCredential::new() as Arc<dyn TokenCredential>),
            Self::ManagedIdentityCredential() => ManagedIdentityCredential::new(None)
                .map(|credential| credential as Arc<dyn TokenCredential>),
            Self::WorkloadIdentityCredential() => WorkloadIdentityCredential::new(None)
                .map(|credential| credential as Arc<dyn TokenCredential>),
            Self::AzureCli() => AzureCliCredential::new(None)
                .map(|credential| credential as Arc<dyn TokenCredential>),
            Self::AzureDeveloperCli() => AzureDeveloperCliCredential::new(None)
                .map(|credential| credential as Arc<dyn TokenCredential>),
            Self::ClientSecret {
                tenant_id,
                client_id,
                client_secret,
            } => ClientSecretCredential::new(
                tenant_id,
                client_id.clone(),
                client_secret.clone().into(),
                None,
            )
            .map(|credential| credential as Arc<dyn TokenCredential>),
        }
        .map_err(|error| {
            PyRuntimeError::new_err(format!("Failed to create Azure credential: {error}"))
        })?;

        Ok(credential)
    }
}
