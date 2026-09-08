use arc_swap::ArcSwap;
use aws_sdk_secretsmanager::Client;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::OnceCell;

use crate::core::{
    ModelRegistry, PythonSettingsProvider, SerdeParser, SettingLookup, SettingsProvider,
};

#[pyclass(extends = PythonSettingsProvider, frozen, str)]
pub struct AwsSecretsManagerSettingsProvider {
    data: ArcSwap<Py<PyDict>>,
    secret_id: String,
    secrets_manager_client: Arc<Client>,
    model_registry: OnceCell<Py<ModelRegistry>>,
}

#[pymethods]
impl AwsSecretsManagerSettingsProvider {
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

impl AwsSecretsManagerSettingsProvider {
    pub fn new(py: Python<'_>, secret_id: String, secrets_manager_client: Arc<Client>) -> Self {
        Self {
            data: ArcSwap::from_pointee(PyDict::new(py).unbind()),
            secret_id,
            secrets_manager_client,
            model_registry: OnceCell::new(),
        }
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
    fn data(&self, py: Python<'_>) -> Py<PyDict> {
        let data = self.data.load();
        data.clone_ref(py)
    }

    async fn reload(&self) -> PyResult<()> {
        let get_secret_value_response = self
            .secrets_manager_client
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
        Self::normalize_keys(&mut parsed_data);
        let data = Python::attach(|py| Self::create_data(py, parsed_data))?;
        self.data.store(Arc::new(data));
        Python::attach(|py| Self::on_reload(py, self.model_registry()));
        Ok(())
    }

    fn model_registry(&self) -> &OnceCell<Py<ModelRegistry>> {
        &self.model_registry
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
    use crate::aws::{
        identity::PythonAwsCredential, secrets_manager::AwsSecretsManagerSettingsSource,
    };
    use crate::core::{ModelRegistry, SettingsProvider};
    use aws_sdk_secretsmanager::Client;
    use pyo3::{
        Py, Python,
        types::{PyAnyMethods, PyModule, PyWeakrefReference},
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    async fn create_secrets_manager_client() -> Arc<Client> {
        Arc::new(
            AwsSecretsManagerSettingsSource::create_secrets_manager_client(
                &PythonAwsCredential::Session {
                    access_key_id: String::from("access-key"),
                    secret_access_key: String::from("secret-key"),
                    session_token: String::from("session-token"),
                },
                Some(String::from("eu-west-1")),
                Some(String::from("http://127.0.0.1:9")),
            )
            .await,
        )
    }

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

    #[tokio::test]
    async fn test_fail_when_loading_secret_and_request_fails() {
        Python::initialize();
        let secrets_manager_client = create_secrets_manager_client().await;

        let provider = Python::attach(|py| {
            AwsSecretsManagerSettingsProvider::new(
                py,
                String::from("dev/secret-id"),
                secrets_manager_client,
            )
        });

        let error = SettingsProvider::reload(&provider).await.unwrap_err();

        assert!(error.to_string().starts_with(
            "RuntimeError: Failed to read AWS secret 'dev/secret-id' from AWS Secrets Manager:"
        ));
    }

    #[tokio::test]
    async fn test_display_returns_type_name() {
        Python::initialize();
        let secrets_manager_client = create_secrets_manager_client().await;

        let display = Python::attach(|py| {
            AwsSecretsManagerSettingsProvider::new(
                py,
                String::from("dev/secret-id"),
                secrets_manager_client,
            )
            .to_string()
        });

        assert_eq!(display, "AwsSecretsManagerSettingsProvider");
    }

    #[tokio::test]
    async fn test_set_model_registry() {
        Python::initialize();
        let secrets_manager_client = create_secrets_manager_client().await;

        Python::attach(|py| {
            let module = PyModule::from_code(py, c"def callback():\n    pass\n", c"", c"").unwrap();
            let callback = module.getattr("callback").unwrap();
            let callback_reference = PyWeakrefReference::new(&callback).unwrap().unbind();
            let model_registry = Py::new(py, ModelRegistry::new(py, callback_reference)).unwrap();
            let provider = AwsSecretsManagerSettingsProvider::new(
                py,
                String::from("dev/secret-id"),
                secrets_manager_client,
            );

            provider
                .set_model_registry(model_registry.bind(py).borrow())
                .unwrap();

            assert!(
                provider
                    .model_registry()
                    .get()
                    .unwrap()
                    .bind(py)
                    .is(model_registry.bind(py))
            );
        });
    }
}
