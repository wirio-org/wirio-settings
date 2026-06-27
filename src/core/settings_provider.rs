use crate::core::convention_changer::ConventionChanger;
use std::collections::HashMap;

pub struct SettingsProvider;

impl SettingsProvider {
    pub fn normalize_loaded_data(data: &mut HashMap<String, Option<String>>) {
        let mut normalized_data: HashMap<String, Option<String>> = HashMap::new();

        for (item_key, item_data) in data.iter() {
            let item_key_in_snake_case = ConventionChanger::to_snake_case(item_key);
            normalized_data.insert(item_key_in_snake_case, item_data.clone());
        }

        *data = normalized_data;
    }
}
