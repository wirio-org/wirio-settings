use pyo3::prelude::*;

mod aws_secrets_manager;
mod azure_key_vault;
mod core;
mod environment_variables;
mod gcp_secret_manager;
mod json_file;
mod key_per_file;
mod yaml_file;

#[pymodule]
mod _wirio_settings {
    #[pymodule_export]
    pub use crate::core::SettingsPath;

    #[pymodule_export]
    pub use crate::core::PythonSettingsProvider;

    #[pymodule_export]
    pub use crate::core::SettingsSource;

    #[pymodule_export]
    pub use crate::core::SettingLookup;

    #[pymodule_export]
    pub use crate::aws_secrets_manager::AwsSecretsManagerSettingsSource;

    #[pymodule_export]
    pub use crate::aws_secrets_manager::AwsSecretsManagerSettingsProvider;

    #[pymodule_export]
    pub use crate::azure_key_vault::AzureKeyVaultSettingsSource;

    #[pymodule_export]
    pub use crate::azure_key_vault::AzureKeyVaultSettingsProvider;

    #[pymodule_export]
    pub use crate::environment_variables::EnvironmentVariablesSettingsSource;

    #[pymodule_export]
    pub use crate::environment_variables::EnvironmentVariablesSettingsProvider;

    #[pymodule_export]
    pub use crate::gcp_secret_manager::GcpSecretManagerSettingsSource;

    #[pymodule_export]
    pub use crate::gcp_secret_manager::GcpSecretManagerSettingsProvider;

    #[pymodule_export]
    pub use crate::json_file::JsonFileSettingsSource;

    #[pymodule_export]
    pub use crate::json_file::JsonFileSettingsProvider;

    #[pymodule_export]
    pub use crate::key_per_file::KeyPerFileSettingsSource;

    #[pymodule_export]
    pub use crate::key_per_file::KeyPerFileSettingsProvider;

    #[pymodule_export]
    pub use crate::yaml_file::YamlFileSettingsSource;

    #[pymodule_export]
    pub use crate::yaml_file::YamlFileSettingsProvider;
}
