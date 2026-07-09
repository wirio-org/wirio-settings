use crate::core::SettingsProvider;
use google_cloud_auth::credentials::Credentials as GoogleCredentials;
use google_cloud_auth::credentials::service_account::Builder as ServiceAccountCredentialsBuilder;
use google_cloud_gax::paginator::ItemPaginator;
use google_cloud_secretmanager_v1::client::SecretManagerService;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

#[pyclass(str)]
pub struct PythonGcpSecretManagerSettingsProvider {
    provider: GcpSecretManagerSettingsProvider,
}

#[pymethods]
impl PythonGcpSecretManagerSettingsProvider {
    #[new]
    #[pyo3(signature = (project_id, credentials_json=None))]
    fn new(project_id: String, credentials_json: Option<String>) -> Self {
        Self {
            provider: GcpSecretManagerSettingsProvider::new(project_id, credentials_json),
        }
    }

    #[getter]
    fn data(&self) -> &BTreeMap<String, Option<String>> {
        &self.provider.data
    }

    pub fn load(&mut self) -> PyResult<()> {
        let runtime = tokio::runtime::Runtime::new().map_err(|error| {
            PyRuntimeError::new_err(format!("Failed to create Tokio runtime: {error}"))
        })?;

        runtime.block_on(self.provider.load())
    }
}

impl fmt::Display for PythonGcpSecretManagerSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GcpSecretManagerSettingsProvider")
    }
}

struct GcpSecretManagerSettingsProvider {
    data: BTreeMap<String, Option<String>>,
    project_id: String,
    credentials_json: Option<String>,
}

impl GcpSecretManagerSettingsProvider {
    fn new(project_id: String, credentials_json: Option<String>) -> Self {
        Self {
            data: BTreeMap::new(),
            project_id,
            credentials_json,
        }
    }

    async fn create_secret_manager_client(&self) -> PyResult<SecretManagerService> {
        let mut secret_manager_client_builder = SecretManagerService::builder();

        if let Some(credentials_json) = self.credentials_json.as_ref() {
            let explicit_credentials = Self::build_explicit_credentials(credentials_json)?;
            secret_manager_client_builder =
                secret_manager_client_builder.with_credentials(explicit_credentials);
        }

        secret_manager_client_builder
            .build()
            .await
            .map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to create GCP Secret Manager client: {error}"
                ))
            })
    }

    fn build_explicit_credentials(credentials_json: &str) -> PyResult<GoogleCredentials> {
        let service_account_key: Value =
            serde_json::from_str(credentials_json).map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to parse GCP credentials JSON from Python credentials object: {error}",
                ))
            })?;

        ServiceAccountCredentialsBuilder::new(service_account_key)
            .build()
            .map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to build explicit GCP service-account credentials: {error}",
                ))
            })
    }

    async fn get_secret_names(
        &self,
        secret_manager_client: &SecretManagerService,
    ) -> PyResult<Vec<String>> {
        let mut secret_names: Vec<String> = Vec::new();
        let mut items = secret_manager_client
            .list_secrets()
            .set_parent(format!("projects/{}", self.project_id))
            .by_item();

        while let Some(item) = items.next().await {
            match item {
                Ok(secret) => {
                    let secret_name = Self::extract_secret_name(&secret.name)?;
                    secret_names.push(secret_name);
                }
                Err(error) => {
                    println!(
                        "Failed to list secrets for project '{}' in GCP Secret Manager: {error}",
                        self.project_id,
                    );
                    return Err(PyRuntimeError::new_err(format!(
                        "Failed to list secrets for project '{}' in GCP Secret Manager: {error}",
                        self.project_id,
                    )));
                }
            }
        }

        Ok(secret_names)
    }

    fn extract_secret_name(secret_resource_name: &str) -> PyResult<String> {
        let secret_name = secret_resource_name.rsplit('/').next().unwrap_or_default();

        if secret_name.is_empty() {
            return Err(PyRuntimeError::new_err(format!(
                "Invalid GCP secret resource name: '{secret_resource_name}'",
            )));
        }

        Ok(String::from(secret_name))
    }

    async fn get_secret_values(
        &self,
        secret_manager_client: &SecretManagerService,
        secret_names: Vec<String>,
    ) -> PyResult<BTreeMap<String, Option<String>>> {
        let mut secret_values = BTreeMap::new();

        for secret_name in secret_names {
            let secret_version_path = format!(
                "projects/{}/secrets/{}/versions/latest",
                self.project_id, secret_name
            );

            let access_secret_version_response = secret_manager_client
            .access_secret_version()
            .set_name(secret_version_path)
            .send()
            .await
            .map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to access secret '{}' in project '{}' from GCP Secret Manager: {error}",
                    secret_name, self.project_id,
                ))
            })?;

            let secret_payload = access_secret_version_response.payload.ok_or_else(|| {
                PyRuntimeError::new_err(format!(
                    "Secret '{}' in project '{}' returned an empty payload",
                    secret_name, self.project_id,
                ))
            })?;

            let secret_value =
                Self::parse_secret_payload_to_string(&secret_name, &secret_payload.data)?;
            secret_values.insert(secret_name, Some(secret_value));
        }

        Ok(secret_values)
    }

    fn parse_secret_payload_to_string(secret_name: &str, data: &[u8]) -> PyResult<String> {
        String::from_utf8(data.to_vec()).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Secret '{secret_name}' in GCP Secret Manager does not contain valid UTF-8 data: {error}",
            ))
        })
    }
}

impl SettingsProvider for GcpSecretManagerSettingsProvider {
    async fn load(&mut self) -> PyResult<()> {
        let secret_manager_client = self.create_secret_manager_client().await?;
        let secret_names = self.get_secret_names(&secret_manager_client).await?;
        let mut secret_values = self
            .get_secret_values(&secret_manager_client, secret_names)
            .await?;
        self.normalize_keys(&mut secret_values);
        self.data = secret_values;
        Ok(())
    }

    fn section_separator() -> Option<&'static str> {
        Some("--")
    }
}

impl fmt::Display for GcpSecretManagerSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}

#[cfg(test)]
mod tests {
    use super::GcpSecretManagerSettingsProvider;
    use crate::core::SettingsProvider;
    use pyo3::Python;

    #[test]
    fn test_replace_double_dash_with_dot_in_secret_name() {
        let normalized_key = GcpSecretManagerSettingsProvider::normalize_section_separator(
            String::from("Logging--LogLevel--Default"),
        );

        assert_eq!(normalized_key, "Logging.LogLevel.Default");
    }

    #[test]
    fn test_extract_secret_name_from_resource_name() {
        let secret_name = GcpSecretManagerSettingsProvider::extract_secret_name(
            "projects/test-project/secrets/ServiceApiKey",
        )
        .unwrap();

        assert_eq!(secret_name, "ServiceApiKey");
    }

    #[test]
    fn test_display_returns_type_name() {
        let display =
            GcpSecretManagerSettingsProvider::new(String::from("my-project-id"), None).to_string();

        assert_eq!(display, "GcpSecretManagerSettingsProvider");
    }

    #[test]
    fn test_fail_when_extracting_secret_name_from_empty_resource_name() {
        Python::initialize();

        let error = GcpSecretManagerSettingsProvider::extract_secret_name("").unwrap_err();

        assert_eq!(
            error.to_string(),
            "RuntimeError: Invalid GCP secret resource name: ''"
        );
    }

    #[test]
    fn test_parse_secret_payload() {
        let secret_value = GcpSecretManagerSettingsProvider::parse_secret_payload_to_string(
            "ApiKey",
            b"my-secret",
        )
        .unwrap();

        assert_eq!(secret_value, "my-secret");
    }

    #[test]
    fn test_fail_parsing_secret_payload_when_value_is_not_valid_utf8() {
        Python::initialize();

        let invalid_utf8_secret_payload = [0xFF];

        let error = GcpSecretManagerSettingsProvider::parse_secret_payload_to_string(
            "ApiKey",
            &invalid_utf8_secret_payload,
        )
        .unwrap_err();

        assert!(error.to_string().contains(
            "RuntimeError: Secret 'ApiKey' in GCP Secret Manager does not contain valid UTF-8 data:"
        ));
    }
}
