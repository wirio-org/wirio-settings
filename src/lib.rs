use pyo3::prelude::*;

mod core;
mod environment_variables;

#[pymodule]
mod _wirio_settings {
    #[pymodule_export]
    pub use crate::core::settings_path::SettingsPath;

    #[pymodule_export]
    pub use crate::core::convention_changer::ConventionChanger;

    #[pymodule_export]
    pub use crate::environment_variables::InternalEnvironmentVariablesSettingsProvider;
}
