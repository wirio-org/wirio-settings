use aws_config::{
    default_provider::credentials::DefaultCredentialsChain,
    environment::EnvironmentVariableCredentialsProvider, profile::ProfileFileCredentialsProvider,
};
use aws_sdk_secretsmanager::config::{Credentials, ProvideCredentials};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "AwsCredential", frozen)]
pub enum PythonAwsCredential {
    Default(),
    Key {
        access_key_id: String,
        secret_access_key: String,
    },
    EnvironmentVariable(),
    #[pyo3(constructor = (profile_name=None))]
    ProfileFile {
        profile_name: Option<String>,
    },
    Session {
        access_key_id: String,
        secret_access_key: String,
        session_token: String,
    },
}

impl PythonAwsCredential {
    pub(crate) async fn to_credential(&self) -> Arc<dyn ProvideCredentials> {
        match self {
            Self::Default() => Arc::new(DefaultCredentialsChain::builder().build().await),
            Self::Key {
                access_key_id,
                secret_access_key,
            } => Arc::new(Credentials::new(
                access_key_id,
                secret_access_key,
                None,
                None,
                "wirio-settings",
            )),
            Self::EnvironmentVariable() => Arc::new(EnvironmentVariableCredentialsProvider::new()),
            Self::ProfileFile { profile_name } => {
                let mut credential_builder = ProfileFileCredentialsProvider::builder();

                if let Some(profile_name) = profile_name {
                    credential_builder = credential_builder.profile_name(profile_name);
                }

                Arc::new(credential_builder.build())
            }
            Self::Session {
                access_key_id,
                secret_access_key,
                session_token,
            } => Arc::new(Credentials::new(
                access_key_id,
                secret_access_key,
                Some(session_token.clone()),
                None,
                "wirio-settings",
            )),
        }
    }
}
