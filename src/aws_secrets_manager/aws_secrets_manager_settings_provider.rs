use aws_config::{BehaviorVersion, Region};
use aws_sdk_secretsmanager::Client;
use aws_sdk_secretsmanager::config::{Builder as SecretsManagerConfigBuilder, Credentials};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

use crate::core::{PythonSettingsProvider, SerdeParser, SettingLookup, SettingsProvider};

#[pyclass(extends = PythonSettingsProvider, str)]
pub struct AwsSecretsManagerSettingsProvider {
    data: BTreeMap<String, Option<String>>,
    secret_id: String,
    region: Option<String>,
    url: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    profile: Option<String>,
}

#[pymethods]
impl AwsSecretsManagerSettingsProvider {
    #[new]
    #[pyo3(signature = (secret_id, region=None, url=None, access_key_id=None, secret_access_key=None, session_token=None, profile=None))]
    #[allow(clippy::too_many_arguments)]
    pub fn new_python(
        secret_id: String,
        region: Option<String>,
        url: Option<String>,
        access_key_id: Option<String>,
        secret_access_key: Option<String>,
        session_token: Option<String>,
        profile: Option<String>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(Self::new(
            secret_id,
            region,
            url,
            access_key_id,
            secret_access_key,
            session_token,
            profile,
        ))
    }

    #[getter]
    fn data(&self) -> &BTreeMap<String, Option<String>> {
        SettingsProvider::data(self)
    }

    fn try_get(&self, key: &str) -> SettingLookup {
        SettingsProvider::try_get(self, key)
    }

    pub fn load_sync(&mut self) -> PyResult<()> {
        SettingsProvider::load_sync(self)
    }
}

impl AwsSecretsManagerSettingsProvider {
    pub fn new(
        secret_id: String,
        region: Option<String>,
        url: Option<String>,
        access_key_id: Option<String>,
        secret_access_key: Option<String>,
        session_token: Option<String>,
        profile: Option<String>,
    ) -> Self {
        Self {
            data: BTreeMap::new(),
            secret_id,
            region,
            url,
            access_key_id,
            secret_access_key,
            session_token,
            profile,
        }
    }

    fn validate_explicit_credentials(&self) -> PyResult<()> {
        if self.has_explicit_credentials()
            && (self.access_key_id.is_none() || self.secret_access_key.is_none())
        {
            return Err(PyRuntimeError::new_err(
                "Both 'access_key_id' and 'secret_access_key' must be provided when using explicit AWS credentials",
            ));
        }

        Ok(())
    }

    fn has_explicit_credentials(&self) -> bool {
        self.access_key_id.is_some()
            || self.secret_access_key.is_some()
            || self.session_token.is_some()
    }

    async fn create_secrets_manager_client(&self) -> PyResult<Client> {
        self.validate_explicit_credentials()?;

        let mut config_loader = aws_config::defaults(BehaviorVersion::latest());

        if let Some(region) = &self.region {
            config_loader = config_loader.region(Region::new(region.clone()));
        }

        if let Some(profile) = &self.profile {
            config_loader = config_loader.profile_name(profile);
        }

        if self.has_explicit_credentials() {
            let credentials = Credentials::new(
                self.access_key_id.clone().ok_or_else(|| {
                    PyRuntimeError::new_err("Missing 'access_key_id' for explicit AWS credentials")
                })?,
                self.secret_access_key.clone().ok_or_else(|| {
                    PyRuntimeError::new_err(
                        "Missing 'secret_access_key' for explicit AWS credentials",
                    )
                })?,
                self.session_token.clone(),
                None,
                "wirio-settings",
            );
            config_loader = config_loader.credentials_provider(credentials);
        }

        let sdk_config = config_loader.load().await;
        let mut secrets_manager_config_builder = SecretsManagerConfigBuilder::from(&sdk_config);

        if let Some(url) = &self.url {
            secrets_manager_config_builder = secrets_manager_config_builder.endpoint_url(url);
        }

        let secrets_manager_config = secrets_manager_config_builder.build();
        Ok(Client::from_conf(secrets_manager_config))
    }

    fn parse_secret_string(secret_string: &str) -> PyResult<BTreeMap<String, Option<String>>> {
        let parsed_secret: Value = serde_json::from_str(secret_string).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Could not parse AWS Secrets Manager secret as JSON: {error}"
            ))
        })?;

        let secret_object = parsed_secret.as_object().ok_or_else(|| {
            PyRuntimeError::new_err("AWS Secrets Manager secret JSON must be an object")
        })?;

        SerdeParser::new().parse(secret_object)
    }
}

impl SettingsProvider for AwsSecretsManagerSettingsProvider {
    fn data(&self) -> &BTreeMap<String, Option<String>> {
        &self.data
    }

    async fn load(&mut self) -> PyResult<()> {
        let secrets_manager_client = self.create_secrets_manager_client().await?;
        let get_secret_value_response = secrets_manager_client
            .get_secret_value()
            .secret_id(&self.secret_id)
            .send()
            .await
            .map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "Failed to read AWS secret '{}' from AWS Secrets Manager: {error}",
                    self.secret_id,
                ))
            })?;
        let secret_string = get_secret_value_response.secret_string().ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "AWS secret '{}' does not contain a string value",
                self.secret_id
            ))
        })?;
        let mut parsed_data = Self::parse_secret_string(secret_string)?;
        self.normalize_keys(&mut parsed_data);
        self.data = parsed_data;
        Ok(())
    }
}

impl fmt::Display for AwsSecretsManagerSettingsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_type_name())
    }
}

#[cfg(test)]
mod tests {
    use super::AwsSecretsManagerSettingsProvider;
    use crate::core::SettingsProvider;
    use pyo3::Python;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn test_parse_secret_string() {
        let expected_data = BTreeMap::from([
            (
                String::from("logging.log_level.default"),
                Some(String::from("WARNING")),
            ),
            (String::from("port"), Some(String::from("8080"))),
            (String::from("enabled"), Some(String::from("true"))),
            (String::from("notes"), None),
            (String::from("items"), Some(String::new())),
        ]);
        let secret = json!({
            "logging": {"log_level": {"default": "WARNING"}},
            "port": 8080,
            "enabled": true,
            "notes": null,
            "items": [],
        })
        .to_string();

        let data = AwsSecretsManagerSettingsProvider::parse_secret_string(&secret).unwrap();

        assert_eq!(data, expected_data);
    }

    #[test]
    fn test_fail_when_secret_json_is_not_object() {
        Python::initialize();

        let secret = json!([1, 2, 3]).to_string();
        let error = AwsSecretsManagerSettingsProvider::parse_secret_string(&secret).unwrap_err();

        assert_eq!(
            error.to_string(),
            "RuntimeError: AWS Secrets Manager secret JSON must be an object"
        );
    }

    #[test]
    fn test_validate_explicit_credentials_require_access_key_and_secret() {
        Python::initialize();

        let provider = AwsSecretsManagerSettingsProvider::new(
            String::from("dev/secret-id"),
            None,
            None,
            Some(String::from("access-key")),
            None,
            None,
            None,
        );

        let error = provider.validate_explicit_credentials().unwrap_err();

        assert_eq!(
            error.to_string(),
            "RuntimeError: Both 'access_key_id' and 'secret_access_key' must be provided when using explicit AWS credentials"
        );
    }

    #[test]
    fn test_fail_when_secret_json_is_invalid() {
        Python::initialize();

        let error =
            AwsSecretsManagerSettingsProvider::parse_secret_string("{invalid-json").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("RuntimeError: Could not parse AWS Secrets Manager secret as JSON:")
        );
    }

    #[test]
    fn test_fail_when_session_token_is_provided_without_access_key_and_secret() {
        Python::initialize();

        let provider = AwsSecretsManagerSettingsProvider::new(
            String::from("dev/secret-id"),
            None,
            None,
            None,
            None,
            Some(String::from("session-token")),
            None,
        );

        let error = provider.validate_explicit_credentials().unwrap_err();

        assert_eq!(
            error.to_string(),
            "RuntimeError: Both 'access_key_id' and 'secret_access_key' must be provided when using explicit AWS credentials"
        );
    }

    #[test]
    fn test_not_validate_explicit_credentials_when_no_explicit_credentials_are_provided() {
        let provider = AwsSecretsManagerSettingsProvider::new(
            String::from("dev/secret-id"),
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let result = provider.validate_explicit_credentials();

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_secrets_manager_client_with_region_profile_url_and_credentials() {
        Python::initialize();

        let provider = AwsSecretsManagerSettingsProvider::new(
            String::from("dev/secret-id"),
            Some(String::from("eu-west-1")),
            Some(String::from("http://127.0.0.1:9")),
            Some(String::from("access-key")),
            Some(String::from("secret-key")),
            Some(String::from("session-token")),
            Some(String::from("integration")),
        );

        let secrets_manager_client_result = provider.create_secrets_manager_client().await;

        assert!(secrets_manager_client_result.is_ok());
    }

    #[tokio::test]
    async fn test_fail_when_loading_secret_and_request_fails() {
        Python::initialize();

        let mut provider = AwsSecretsManagerSettingsProvider::new(
            String::from("dev/secret-id"),
            Some(String::from("eu-west-1")),
            Some(String::from("http://127.0.0.1:9")),
            Some(String::from("access-key")),
            Some(String::from("secret-key")),
            None,
            None,
        );

        let error = SettingsProvider::load(&mut provider).await.unwrap_err();

        assert!(error.to_string().starts_with(
            "RuntimeError: Failed to read AWS secret 'dev/secret-id' from AWS Secrets Manager:"
        ));
    }

    #[test]
    fn test_display_returns_type_name() {
        let display = AwsSecretsManagerSettingsProvider::new(
            String::from("dev/secret-id"),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .to_string();

        assert_eq!(display, "AwsSecretsManagerSettingsProvider");
    }
}
