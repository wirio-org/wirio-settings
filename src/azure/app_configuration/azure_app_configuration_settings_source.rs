use crate::{
    azure::{
        app_configuration::{
            AzureAppConfigurationSettingsProvider,
            azure_app_configuration_client::AzureAppConfigurationClient,
            models::{FeatureFlagSelector, KeyFilter, SettingSelector},
        },
        identity::PythonAzureCredential,
    },
    core::{PythonSettingsProvider, PythonSettingsSource, SettingsSource},
};
use azure_core::credentials::TokenCredential;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(
    name = "_AzureAppConfigurationSettingsSource",
    extends = PythonSettingsSource,
    frozen
)]
pub struct AzureAppConfigurationSettingsSource {
    pub(crate) endpoint: String,
    pub(crate) client: Arc<AzureAppConfigurationClient>,
    pub(crate) credential: Arc<dyn TokenCredential>,
    pub(crate) selectors: Vec<SettingSelector>,
    pub(crate) trim_key_prefixes: Vec<String>,
    pub(crate) feature_flag_selectors: Vec<FeatureFlagSelector>,
    pub(crate) feature_flag_trim_name_prefixes: Vec<String>,
}

#[pymethods]
impl AzureAppConfigurationSettingsSource {
    #[new]
    #[pyo3(signature = (endpoint, credential, selectors=None, trim_key_prefixes=None, feature_flag_selectors=None, feature_flag_trim_name_prefixes=None))]
    pub fn new_python(
        endpoint: String,
        credential: &PythonAzureCredential,
        selectors: Option<Vec<SettingSelector>>,
        trim_key_prefixes: Option<Vec<String>>,
        feature_flag_selectors: Option<Vec<FeatureFlagSelector>>,
        feature_flag_trim_name_prefixes: Option<Vec<String>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let credential = credential.to_token_credential()?;
        let client = Self::create_client(&endpoint, Arc::clone(&credential))?;
        Ok(
            PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
                endpoint,
                client: Arc::new(client),
                credential,
                selectors: Self::get_selectors_or_default(selectors),
                trim_key_prefixes: trim_key_prefixes.unwrap_or_default(),
                feature_flag_selectors: Self::get_feature_flag_selectors_or_default(
                    feature_flag_selectors,
                ),
                feature_flag_trim_name_prefixes: feature_flag_trim_name_prefixes
                    .unwrap_or_default(),
            }),
        )
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl AzureAppConfigurationSettingsSource {
    fn create_client(
        endpoint: &str,
        credential: Arc<dyn TokenCredential>,
    ) -> PyResult<AzureAppConfigurationClient> {
        AzureAppConfigurationClient::new(endpoint, credential).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "Failed to create Azure App Configuration client for '{endpoint}': {error}",
            ))
        })
    }

    fn get_selectors_or_default(selectors: Option<Vec<SettingSelector>>) -> Vec<SettingSelector> {
        selectors.unwrap_or_else(|| vec![SettingSelector::new(String::from(KeyFilter::ANY), None)])
    }

    fn get_feature_flag_selectors_or_default(
        feature_flag_selectors: Option<Vec<FeatureFlagSelector>>,
    ) -> Vec<FeatureFlagSelector> {
        feature_flag_selectors
            .unwrap_or_else(|| vec![FeatureFlagSelector::new(String::from(KeyFilter::ANY), None)])
    }
}

impl SettingsSource for AzureAppConfigurationSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new())
                .add_subclass(AzureAppConfigurationSettingsProvider::new(py, self)),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use super::AzureAppConfigurationSettingsSource;
    use crate::azure::{
        app_configuration::models::{KeyFilter, LabelFilter},
        identity::PythonAzureCredential,
    };
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;
    use std::sync::Arc;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let python_credential = PythonAzureCredential::ClientSecret {
                tenant_id: String::from("tenant-id"),
                client_id: String::from("client-id"),
                client_secret: String::from("client-secret"),
            };
            let credential = python_credential.to_token_credential().unwrap();
            let source = AzureAppConfigurationSettingsSource {
                endpoint: String::from("https://example.azconfig.io"),
                credential: Arc::clone(&credential),
                selectors: Vec::new(),
                trim_key_prefixes: Vec::new(),
                feature_flag_selectors: Vec::new(),
                feature_flag_trim_name_prefixes: Vec::new(),
                client: Arc::new(
                    AzureAppConfigurationSettingsSource::create_client(
                        "https://example.azconfig.io",
                        credential,
                    )
                    .unwrap(),
                ),
            };

            let provider = source.build(py).unwrap();

            assert!(provider.bind(py).is_instance_of::<
                crate::azure::app_configuration::AzureAppConfigurationSettingsProvider,
            >());
        });
    }

    #[test]
    fn test_get_default_selectors_when_none_provided() {
        let selectors = AzureAppConfigurationSettingsSource::get_selectors_or_default(None);
        let feature_flag_selectors =
            AzureAppConfigurationSettingsSource::get_feature_flag_selectors_or_default(None);

        assert_eq!(selectors[0].key_filter, KeyFilter::ANY);
        assert_eq!(selectors[0].label_filter, LabelFilter::NULL);
        assert_eq!(feature_flag_selectors[0].name_filter, KeyFilter::ANY);
        assert_eq!(feature_flag_selectors[0].label_filter, LabelFilter::NULL);
    }
}
