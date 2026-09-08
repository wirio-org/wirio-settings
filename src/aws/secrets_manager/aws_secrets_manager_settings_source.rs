use crate::{
    aws::{identity::PythonAwsCredential, secrets_manager::AwsSecretsManagerSettingsProvider},
    core::{PythonSettingsProvider, PythonSettingsSource, SettingsSource},
};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_secretsmanager::{Client, config::Builder};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(extends = PythonSettingsSource, frozen)]
pub struct AwsSecretsManagerSettingsSource {
    secret_id: String,
    secrets_manager_client: Arc<Client>,
}

#[pymethods]
impl AwsSecretsManagerSettingsSource {
    #[new]
    #[pyo3(signature = (secret_id, credential, region=None, url=None))]
    pub fn new_python(
        py: Python<'_>,
        secret_id: String,
        credential: &PythonAwsCredential,
        region: Option<String>,
        url: Option<String>,
    ) -> PyClassInitializer<Self> {
        let secrets_manager_client = py.detach(|| {
            pyo3_async_runtimes::tokio::get_runtime()
                .block_on(Self::create_secrets_manager_client(credential, region, url))
        });

        PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
            secret_id,
            secrets_manager_client: Arc::new(secrets_manager_client),
        })
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl AwsSecretsManagerSettingsSource {
    pub(crate) async fn create_secrets_manager_client(
        credential_type: &PythonAwsCredential,
        region: Option<String>,
        url: Option<String>,
    ) -> Client {
        let credential = credential_type.to_credential().await;
        let mut config_loader = aws_config::defaults(BehaviorVersion::latest());

        if let Some(region) = region {
            config_loader = config_loader.region(Region::new(region));
        }

        let sdk_config = config_loader.credentials_provider(credential).load().await;
        let mut secrets_manager_config_builder = Builder::from(&sdk_config);

        if let Some(url) = url {
            secrets_manager_config_builder = secrets_manager_config_builder.endpoint_url(url);
        }

        Client::from_conf(secrets_manager_config_builder.build())
    }
}

impl SettingsSource for AwsSecretsManagerSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                AwsSecretsManagerSettingsProvider::new(
                    py,
                    self.secret_id.clone(),
                    Arc::clone(&self.secrets_manager_client),
                ),
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use super::AwsSecretsManagerSettingsSource;
    use crate::aws::identity::PythonAwsCredential;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_build_provider() {
        Python::initialize();
        let secrets_manager_client =
            AwsSecretsManagerSettingsSource::create_secrets_manager_client(
                &PythonAwsCredential::Key {
                    access_key_id: String::from("access-key"),
                    secret_access_key: String::from("secret-key"),
                },
                None,
                None,
            )
            .await;

        Python::attach(|py| {
            let source = AwsSecretsManagerSettingsSource {
                secret_id: String::from("settings"),
                secrets_manager_client: Arc::new(secrets_manager_client),
            };

            let provider = source.build(py).unwrap();

            assert!(provider
                .bind(py)
                .is_instance_of::<crate::aws::secrets_manager::AwsSecretsManagerSettingsProvider>());
        });
    }
}
