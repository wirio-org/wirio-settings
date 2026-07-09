use pyo3::prelude::*;

mod aws_secrets_manager;
mod core;
mod environment_variables;
mod gcp_secret_manager;
mod json;
mod yaml;

#[pymodule]
mod _wirio_settings {
    #[pymodule_export]
    pub use crate::core::SettingsPath;

    #[pymodule_export]
    pub use crate::core::ConventionChanger;

    #[pymodule_export]
    pub use crate::aws_secrets_manager::PythonAwsSecretsManagerSettingsProvider;

    #[pymodule_export]
    pub use crate::environment_variables::PythonEnvironmentVariablesSettingsProvider;

    #[pymodule_export]
    pub use crate::gcp_secret_manager::PythonGcpSecretManagerSettingsProvider;

    #[pymodule_export]
    pub use crate::json::PythonJsonFileSettingsProvider;

    #[pymodule_export]
    pub use crate::yaml::PythonYamlFileSettingsProvider;
}
