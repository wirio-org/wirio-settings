use pyo3::prelude::*;

#[pyclass]
pub struct SettingsPath;

#[pymethods]
impl SettingsPath {
    #[classattr]
    pub const KEY_DELIMITER: &'static str = ".";

    #[staticmethod]
    pub fn get_section_key(path: &str) -> &str {
        match path.rfind(Self::KEY_DELIMITER) {
            Some(index) => &path[index + Self::KEY_DELIMITER.len()..],
            None => path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsPath;

    #[test]
    fn test_get_last_section_when_path_has_multiple_sections() {
        let key = SettingsPath::get_section_key("logging.log_level.default");

        assert_eq!(key, "default");
    }

    #[test]
    fn test_return_original_path_when_key_has_no_delimiter() {
        let key = SettingsPath::get_section_key("log_level");

        assert_eq!(key, "log_level");
    }

    #[test]
    fn test_return_empty_key_when_path_ends_with_delimiter() {
        let key = SettingsPath::get_section_key("logging.");

        assert_eq!(key, "");
    }
}
