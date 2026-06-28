use crate::core::convention_changer::ConventionChanger;
use std::collections::BTreeMap;

pub struct SettingsProvider;

impl SettingsProvider {
    pub fn normalize_loaded_data(data: &mut BTreeMap<String, Option<String>>) {
        let mut normalized_data: BTreeMap<String, Option<String>> = BTreeMap::new();

        for (item_key, item_data) in data.iter() {
            let item_key_in_snake_case = ConventionChanger::to_snake_case(item_key);
            normalized_data.insert(item_key_in_snake_case, item_data.clone());
        }

        *data = normalized_data;
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsProvider;
    use std::collections::BTreeMap;

    #[test]
    fn test_normalize_loaded_data_converts_keys_to_snake_case() {
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

        SettingsProvider::normalize_loaded_data(&mut data);

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
    fn test_normalize_loaded_data_keeps_none_values() {
        let expected_data = BTreeMap::from([(String::from("connection_string"), None)]);
        let mut data = BTreeMap::from([(String::from("ConnectionString"), None)]);

        SettingsProvider::normalize_loaded_data(&mut data);

        assert_eq!(data, expected_data);
    }

    #[test]
    fn test_normalize_loaded_data_keeps_empty_map_unchanged() {
        let mut data: BTreeMap<String, Option<String>> = BTreeMap::new();

        SettingsProvider::normalize_loaded_data(&mut data);

        assert!(data.is_empty());
    }
}
