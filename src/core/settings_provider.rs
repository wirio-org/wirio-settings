use crate::{_wirio_settings::SettingsPath, core::convention_changer::ConventionChanger};
use pyo3::PyResult;
use std::{collections::BTreeMap, fmt, mem};

pub trait SettingsProvider: fmt::Display {
    async fn load(&self) -> PyResult<BTreeMap<String, Option<String>>>;

    fn normalize_keys(&self, data: &mut BTreeMap<String, Option<String>>) {
        if data.is_empty() {
            return;
        }

        let original_data = mem::take(data);
        let mut normalized_data: BTreeMap<String, Option<String>> = BTreeMap::new();

        for (item_key, item_value) in original_data {
            let item_key_with_normalized_section_separator =
                self.normalize_section_separator(item_key);
            let item_key_in_snake_case =
                ConventionChanger::to_snake_case(&item_key_with_normalized_section_separator);
            normalized_data.insert(item_key_in_snake_case, item_value);
        }

        *data = normalized_data;
    }

    fn normalize_section_separator(&self, key: String) -> String {
        let Some(section_separator) = self.section_separator() else {
            return key;
        };
        key.replace(section_separator, SettingsPath::KEY_DELIMITER)
    }

    fn section_separator(&self) -> Option<&str> {
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
    use super::SettingsProvider;
    use mockall::mock;
    use pyo3::PyResult;
    use std::{collections::BTreeMap, fmt};

    mock! {
        SettingsProvider {}

        impl SettingsProvider for SettingsProvider {
            async fn load(&self) -> PyResult<BTreeMap<String, Option<String>>>;
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
    fn test_return_type_name() {
        let settings_provider_mock = MockSettingsProvider::new();

        let type_name = settings_provider_mock.get_type_name();

        assert_eq!(type_name, "MockSettingsProvider");
    }
}
