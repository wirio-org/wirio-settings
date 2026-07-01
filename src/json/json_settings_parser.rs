use pyo3::PyResult;
use pyo3::exceptions::PyRuntimeError;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

use crate::_wirio_settings::SettingsPath;

pub struct JsonSettingsParser {
    data: BTreeMap<String, Option<String>>,
    paths: Vec<String>,
}

impl JsonSettingsParser {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            paths: Vec::new(),
        }
    }

    pub fn parse(
        mut self,
        json_object: &Map<String, Value>,
    ) -> PyResult<BTreeMap<String, Option<String>>> {
        self.visit_object_element(json_object)?;
        Ok(self.data)
    }

    fn visit_object_element(&mut self, object: &Map<String, Value>) -> PyResult<()> {
        let mut is_empty = true;

        for (key, value) in object {
            is_empty = false;
            self.enter_context(key);
            self.visit_value(value)?;
            self.exit_context();
        }

        self.set_none_if_object_is_empty(is_empty);
        Ok(())
    }

    fn visit_value(&mut self, value: &Value) -> PyResult<()> {
        match value {
            Value::Object(object) => {
                self.visit_object_element(object)?;
            }
            Value::Array(array) => {
                self.visit_array_element(array)?;
            }
            Value::Bool(boolean) => {
                self.set_scalar_value(Some(boolean.to_string()))?;
            }
            Value::Number(number) => {
                self.set_scalar_value(Some(number.to_string()))?;
            }
            Value::String(string) => {
                self.set_scalar_value(Some(string.clone()))?;
            }
            Value::Null => {
                self.set_scalar_value(None)?;
            }
        }

        Ok(())
    }

    fn visit_array_element(&mut self, array: &[Value]) -> PyResult<()> {
        let mut index = 0_usize;

        for item in array {
            self.enter_context(&index.to_string());
            self.visit_value(item)?;
            self.exit_context();
            index += 1;
        }

        let is_empty = index == 0;
        self.set_empty_if_array_is_empty(is_empty);
        Ok(())
    }

    fn set_scalar_value(&mut self, value: Option<String>) -> PyResult<()> {
        let key = self
            .paths
            .last()
            .cloned()
            .ok_or_else(|| PyRuntimeError::new_err("Internal parser context is missing"))?;
        self.data.insert(key, value);
        Ok(())
    }

    fn set_none_if_object_is_empty(&mut self, is_empty: bool) {
        if is_empty && !self.paths.is_empty() {
            let key = self.paths.last().cloned().unwrap();
            self.data.insert(key, None);
        }
    }

    fn set_empty_if_array_is_empty(&mut self, is_empty: bool) {
        if is_empty && !self.paths.is_empty() {
            let key = self.paths.last().cloned().unwrap();
            self.data.insert(key, Some(String::new()));
        }
    }

    fn enter_context(&mut self, context: &str) {
        let new_path = match self.paths.last() {
            Some(path) => format!("{}{}{}", path, SettingsPath::KEY_DELIMITER, context),
            None => String::from(context),
        };
        self.paths.push(new_path);
    }

    fn exit_context(&mut self) {
        self.paths.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::JsonSettingsParser;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn test_parse_scalar_values() {
        let expected_parsed_json = BTreeMap::from([
            (String::from("name"), Some(String::from("wirio"))),
            (String::from("port"), Some(String::from("8080"))),
            (String::from("enabled"), Some(String::from("true"))),
            (String::from("notes"), None),
            (String::from("price"), Some(String::from("19.99"))),
        ]);
        let json = json!({
            "name": "wirio",
            "port": 8080,
            "enabled": true,
            "notes": null,
            "price": 19.99
        });

        let parsed_json = JsonSettingsParser::new()
            .parse(json.as_object().unwrap())
            .unwrap();

        assert_eq!(parsed_json, expected_parsed_json);
    }

    #[test]
    fn test_parse_nested_objects_and_arrays() {
        let expected_parsed_json = BTreeMap::from([
            (
                String::from("Logging.LogLevel.Default"),
                Some(String::from("Information")),
            ),
            (
                String::from("AllowedHosts.0"),
                Some(String::from("localhost")),
            ),
            (
                String::from("AllowedHosts.1"),
                Some(String::from("example.com")),
            ),
        ]);
        let json = json!({
            "Logging": {"LogLevel": {"Default": "Information"}},
            "AllowedHosts": ["localhost", "example.com"]
        });

        let parsed_json = JsonSettingsParser::new()
            .parse(json.as_object().unwrap())
            .unwrap();

        assert_eq!(parsed_json, expected_parsed_json);
    }

    #[test]
    fn test_set_none_and_empty_for_empty_structures() {
        let expected_parsed_json = BTreeMap::from([
            (String::from("Section"), None),
            (String::from("NestedSection.Section"), None),
            (String::from("Items"), Some(String::new())),
            (String::from("NestedItems.Items"), Some(String::new())),
        ]);
        let json = json!({
            "Section": {},
            "NestedSection": {"Section": {}},
            "Items": [],
            "NestedItems": {"Items": []}
        });

        let parsed_json = JsonSettingsParser::new()
            .parse(json.as_object().unwrap())
            .unwrap();

        assert_eq!(parsed_json, expected_parsed_json);
    }
}
