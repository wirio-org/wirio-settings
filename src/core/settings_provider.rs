use crate::{_wirio_settings::SettingsPath, core::convention_changer};
use pyo3::prelude::*;
use std::{collections::BTreeMap, fmt, mem};

use super::SettingLookup;

/// Provides setting values
#[pyclass(subclass, name = "SettingsProvider")]
pub struct PythonSettingsProvider;

#[pymethods]
impl PythonSettingsProvider {
    #[new]
    pub fn new() -> Self {
        Self
    }

    #[getter]
    #[allow(clippy::unused_self)]
    fn data(&self) -> &BTreeMap<String, Option<String>> {
        unimplemented!()
    }

    #[allow(clippy::unused_self)]
    fn try_get(&self, _key: &str) -> SettingLookup {
        unimplemented!()
    }

    #[allow(clippy::unused_self)]
    fn load_sync(&mut self) {
        unimplemented!()
    }
}

pub trait SettingsProvider: fmt::Display {
    fn data(&self) -> &BTreeMap<String, Option<String>>;

    fn try_get(&self, key: &str) -> SettingLookup {
        match self.data().get(key) {
            Some(value) => SettingLookup::Found {
                value: value.clone(),
            },
            None => SettingLookup::Missing(),
        }
    }

    async fn load(&mut self) -> PyResult<()>;

    fn load_sync(&mut self) -> PyResult<()> {
        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        runtime.block_on(self.load())
    }

    fn normalize_keys(&self, data: &mut BTreeMap<String, Option<String>>) {
        if data.is_empty() {
            return;
        }

        let original_data = mem::take(data);
        let mut normalized_data: BTreeMap<String, Option<String>> = BTreeMap::new();

        for (item_key, item_value) in original_data {
            let item_key_with_normalized_section_separator =
                Self::normalize_section_separator(item_key);
            let item_key_in_snake_case =
                convention_changer::to_snake_case(&item_key_with_normalized_section_separator);
            normalized_data.insert(item_key_in_snake_case, item_value);
        }

        *data = normalized_data;
    }

    fn normalize_section_separator(key: String) -> String {
        let Some(section_separator) = Self::section_separator() else {
            return key;
        };
        key.replace(section_separator, SettingsPath::KEY_DELIMITER)
    }

    fn section_separator() -> Option<&'static str> {
        None
    }

    fn get_type_name(&self) -> &str {
        let full_name = std::any::type_name::<Self>();
        let short_name = full_name.split("::").last();
        short_name.unwrap_or(full_name)
    }
}

#[cfg(test)]
mod tests {
    use super::{SettingLookup, SettingsProvider};
    use mockall::mock;
    use pyo3::PyResult;
    use std::{collections::BTreeMap, fmt};

    struct TestSettingsProvider {
        data: BTreeMap<String, Option<String>>,
        is_loaded: bool,
    }

    impl SettingsProvider for TestSettingsProvider {
        fn data(&self) -> &BTreeMap<String, Option<String>> {
            &self.data
        }

        async fn load(&mut self) -> PyResult<()> {
            self.is_loaded = true;
            Ok(())
        }
    }

    impl fmt::Display for TestSettingsProvider {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("TestSettingsProvider")
        }
    }

    mock! {
        SettingsProvider {}

        impl SettingsProvider for SettingsProvider {
            async fn load(&mut self) -> PyResult<()>;
            fn data(&self) -> &BTreeMap<String, Option<String>>;
        }
    }

    impl fmt::Display for MockSettingsProvider {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.get_type_name())
        }
    }

    #[test]
    fn test_convert_keys_to_snake_case_when_normalizing_keys() {
        let settings_provider_mock = MockSettingsProvider::new();
        let mut data = BTreeMap::from([
            (
                String::from("FeatureFlagEnabled"),
                Some(String::from("true")),
            ),
            (
                String::from("LOGGING.LOG_LEVEL.DEFAULT"),
                Some(String::from("WARNING")),
            ),
        ]);

        settings_provider_mock.normalize_keys(&mut data);

        let expected = BTreeMap::from([
            (
                String::from("feature_flag_enabled"),
                Some(String::from("true")),
            ),
            (
                String::from("logging.log_level.default"),
                Some(String::from("WARNING")),
            ),
        ]);

        assert_eq!(data, expected);
    }

    #[test]
    fn test_keep_none_values_when_normalizing_keys() {
        let settings_provider_mock = MockSettingsProvider::new();
        let expected_data = BTreeMap::from([(String::from("connection_string"), None)]);
        let mut data = BTreeMap::from([(String::from("ConnectionString"), None)]);

        settings_provider_mock.normalize_keys(&mut data);

        assert_eq!(data, expected_data);
    }

    #[test]
    fn test_normalize_loaded_data_keeps_empty_map_unchanged() {
        let settings_provider_mock = MockSettingsProvider::new();
        let mut data: BTreeMap<String, Option<String>> = BTreeMap::new();

        settings_provider_mock.normalize_keys(&mut data);

        assert!(data.is_empty());
    }

    #[test]
    fn test_return_loaded_data() {
        let data = BTreeMap::from([(String::from("setting"), Some(String::from("value")))]);
        let settings_provider = TestSettingsProvider {
            data: data.clone(),
            is_loaded: false,
        };

        assert_eq!(SettingsProvider::data(&settings_provider), &data);
    }

    #[test]
    fn test_return_type_name() {
        let settings_provider_mock = MockSettingsProvider::new();

        let type_name = settings_provider_mock.get_type_name();

        assert_eq!(type_name, "MockSettingsProvider");
    }

    #[test]
    fn test_load_synchronously() {
        let mut settings_provider = TestSettingsProvider {
            data: BTreeMap::new(),
            is_loaded: false,
        };

        settings_provider.load_sync().unwrap();

        assert!(settings_provider.is_loaded);
    }

    #[test]
    fn test_return_found_value_when_key_exists() {
        let key = "setting_key";
        let expected_value = "setting_value";
        let settings_provider = TestSettingsProvider {
            data: BTreeMap::from([(String::from(key), Some(String::from(expected_value)))]),
            is_loaded: false,
        };

        let lookup = settings_provider.try_get(key);

        assert!(matches!(
            lookup,
            SettingLookup::Found {
                value: Some(value)
            } if value == expected_value
        ));
    }

    #[test]
    fn test_return_missing_value_when_key_does_not_exist() {
        let settings_provider = TestSettingsProvider {
            data: BTreeMap::new(),
            is_loaded: false,
        };

        let lookup = settings_provider.try_get("setting_key");

        assert!(matches!(lookup, SettingLookup::Missing()));
    }
}
