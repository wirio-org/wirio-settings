mod aws;
mod azure;
mod core;
mod environment_variables;
mod gcp;
mod json_file;
mod setting_per_file;
mod yaml_file;

use pyo3::prelude::*;

#[pymodule]
mod _wirio_settings {
    use pyo3::prelude::*;

    #[pymodule]
    mod core {
        #[pymodule_export]
        pub use crate::core::ModelRegistry;

        #[pymodule_export]
        pub use crate::core::PythonSettingsProvider;

        #[pymodule_export]
        pub use crate::core::PythonSettingsSource;

        #[pymodule_export]
        pub use crate::core::RegisteredModel;

        #[pymodule_export]
        pub use crate::core::SettingLookup;

        #[pymodule_export]
        pub use crate::core::SettingsPath;
    }

    #[pymodule]
    mod environment_variables {
        #[pymodule_export]
        pub use crate::environment_variables::EnvironmentVariablesSettingsProvider;

        #[pymodule_export]
        pub use crate::environment_variables::EnvironmentVariablesSettingsSource;
    }

    #[pymodule]
    mod yaml_file {
        #[pymodule_export]
        pub use crate::yaml_file::YamlFileSettingsProvider;

        #[pymodule_export]
        pub use crate::yaml_file::YamlFileSettingsSource;
    }

    #[pymodule]

    mod json_file {
        #[pymodule_export]
        pub use crate::json_file::JsonFileSettingsProvider;

        #[pymodule_export]
        pub use crate::json_file::JsonFileSettingsSource;
    }

    #[pymodule]
    mod setting_per_file {
        #[pymodule_export]
        pub use crate::setting_per_file::SettingPerFileSettingsProvider;

        #[pymodule_export]
        pub use crate::setting_per_file::SettingPerFileSettingsSource;
    }

    #[pymodule]
    mod azure_identity {
        #[pymodule_export]
        pub use crate::azure::identity::PythonAzureCredential;
    }

    #[pymodule]
    mod azure_key_vault {
        #[pymodule_export]
        pub use crate::azure::key_vault::AzureKeyVaultSettingsProvider;

        #[pymodule_export]
        pub use crate::azure::key_vault::AzureKeyVaultSettingsSource;
    }

    #[pymodule]
    mod azure_app_configuration {
        #[pymodule_export]
        pub use crate::azure::app_configuration::AzureAppConfigurationSettingsProvider;

        #[pymodule_export]
        pub use crate::azure::app_configuration::AzureAppConfigurationSettingsSource;

        #[pymodule_export]
        pub use crate::azure::app_configuration::FeatureFlagSelector;

        #[pymodule_export]
        pub use crate::azure::app_configuration::KeyFilter;

        #[pymodule_export]
        pub use crate::azure::app_configuration::LabelFilter;

        #[pymodule_export]
        pub use crate::azure::app_configuration::NameFilter;

        #[pymodule_export]
        pub use crate::azure::app_configuration::SettingSelector;
    }

    #[pymodule]
    mod aws_identity {
        #[pymodule_export]
        pub use crate::aws::identity::PythonAwsCredential;
    }

    #[pymodule]
    mod aws_secrets_manager {
        #[pymodule_export]
        pub use crate::aws::secrets_manager::AwsSecretsManagerSettingsProvider;

        #[pymodule_export]
        pub use crate::aws::secrets_manager::AwsSecretsManagerSettingsSource;
    }

    #[pymodule]
    mod gcp_secret_manager {
        #[pymodule_export]
        pub use crate::gcp::secret_manager::GcpSecretManagerSettingsProvider;

        #[pymodule_export]
        pub use crate::gcp::secret_manager::GcpSecretManagerSettingsSource;
    }
}
