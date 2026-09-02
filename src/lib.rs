use pyo3::prelude::*;

mod aws_secrets_manager;
mod azure_key_vault;
mod core;
mod environment_variables;
mod gcp_secret_manager;
mod json_file;
mod setting_per_file;
mod yaml_file;

#[pymodule]
mod _wirio_settings {
    #[pymodule_export]
    pub use crate::core::SettingsPath;

    #[pymodule_export]
    pub use crate::core::PythonSettingsProvider;

    #[pymodule_export]
    pub use crate::core::ModelRegistry;

    #[pymodule_export]
    pub use crate::core::RegisteredModel;

    #[pymodule_export]
    pub use crate::core::PythonSettingsSource;

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
    pub use crate::setting_per_file::SettingPerFileSettingsSource;

    #[pymodule_export]
    pub use crate::setting_per_file::SettingPerFileSettingsProvider;

    #[pymodule_export]
    pub use crate::yaml_file::YamlFileSettingsSource;

    #[pymodule_export]
    pub use crate::yaml_file::YamlFileSettingsProvider;
}
