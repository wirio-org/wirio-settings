use crate::{
    aws_secrets_manager::AwsSecretsManagerSettingsProvider,
    core::{PythonSettingsProvider, PythonSettingsSource, SettingsSource},
};
use pyo3::prelude::*;

#[pyclass(extends = PythonSettingsSource)]
pub struct AwsSecretsManagerSettingsSource {
    secret_id: String,
    region: Option<String>,
    url: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    profile: Option<String>,
}

#[pymethods]
impl AwsSecretsManagerSettingsSource {
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
        PyClassInitializer::from(PythonSettingsSource::new()).add_subclass(Self {
            secret_id,
            region,
            url,
            access_key_id,
            secret_access_key,
            session_token,
            profile,
        })
    }

    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        <Self as SettingsSource>::build(self, py)
    }
}

impl SettingsSource for AwsSecretsManagerSettingsSource {
    fn build(&self, py: Python<'_>) -> PyResult<Py<PythonSettingsProvider>> {
        Py::new(
            py,
            PyClassInitializer::from(PythonSettingsProvider::new()).add_subclass(
                AwsSecretsManagerSettingsProvider::new(
                    self.secret_id.clone(),
                    self.region.clone(),
                    self.url.clone(),
                    self.access_key_id.clone(),
                    self.secret_access_key.clone(),
                    self.session_token.clone(),
                    self.profile.clone(),
                ),
            ),
        )
        .map(|provider| provider.into_bound(py).into_super().unbind())
    }
}

#[cfg(test)]
mod tests {
    use super::AwsSecretsManagerSettingsSource;
    use pyo3::Python;
    use pyo3::types::PyAnyMethods;

    #[test]
    fn test_build_provider() {
        Python::initialize();
        Python::attach(|py| {
            let source = AwsSecretsManagerSettingsSource {
                secret_id: String::from("settings"),
                region: None,
                url: None,
                access_key_id: None,
                secret_access_key: None,
                session_token: None,
                profile: None,
            };

            let provider = source.build(py).unwrap();

            assert!(provider
                .bind(py)
                .is_instance_of::<crate::aws_secrets_manager::AwsSecretsManagerSettingsProvider>());
        });
    }
}
