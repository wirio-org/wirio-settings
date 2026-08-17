use crate::{_wirio_settings::SettingsPath, core::convention_changer};
use pyo3::{
    prelude::*,
    types::{PyDict, PyString},
};
use std::{collections::BTreeMap, fmt, mem};

use super::SettingLookup;

/// Provides setting values
#[pyclass(name = "SettingsProvider", subclass, frozen)]
pub struct PythonSettingsProvider;

#[pymethods]
impl PythonSettingsProvider {
    #[new]
    pub fn new() -> Self {
        Self
    }

    #[pyo3(signature = () -> "dict[str, str | None]")]
    #[allow(clippy::unused_self)]
    fn data(&self) -> BTreeMap<String, Option<String>> {
        unimplemented!()
    }

    #[allow(clippy::unused_self)]
    #[allow(unused_variables)]
    fn try_get(&self, key: &str) -> PyResult<SettingLookup> {
        unimplemented!()
    }

    #[allow(clippy::unused_self)]
    fn load(&self) {
        unimplemented!()
    }
}

pub trait SettingsProvider: Sync + fmt::Display {
    fn data(&self, py: Python<'_>) -> Py<PyDict>;

    fn create_data(
        py: Python<'_>,
        source: BTreeMap<String, Option<String>>,
    ) -> PyResult<Py<PyDict>> {
        let data = PyDict::new(py);

        for (key, value) in source {
            data.set_item(key, value)?;
        }

        Ok(data.unbind())
    }

    fn try_get(&self, py: Python<'_>, key: &str) -> PyResult<SettingLookup> {
        let data = self.data(py);

        match data.bind(py).get_item(key)? {
            Some(value) => Ok(SettingLookup::Found {
                value: value.extract::<Option<Py<PyString>>>()?,
            }),
            None => Ok(SettingLookup::Missing()),
        }
    }

    /// Reloads the provider's settings data, or loads it for the first time if it hasn't been loaded yet.
    async fn reload(&self) -> PyResult<()>;

    /// Loads the provider's settings data for the first time and waits for completion.
    ///
    /// This uses [`Self::reload`], so the same loading implementation is shared.
    fn load(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let runtime = pyo3_async_runtimes::tokio::get_runtime();
            runtime.block_on(self.reload())
        })
    }

    fn normalize_keys(data: &mut BTreeMap<String, Option<String>>) {
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
    use super::PythonSettingsProvider;

    use super::{SettingLookup, SettingsProvider};
    use pyo3::{prelude::*, types::PyDict};
    use std::{
        collections::BTreeMap,
        fmt,
        sync::atomic::{AtomicBool, Ordering},
    };

    #[pyclass(extends = PythonSettingsProvider, frozen, str)]
    struct MockSettingsProvider {
        data: Py<PyDict>,
        is_loaded: AtomicBool,
    }

    impl MockSettingsProvider {
        fn new(py: Python<'_>, data: Py<PyDict>) -> Self {
            let _ = py;
            Self {
                data,
                is_loaded: AtomicBool::new(false),
            }
        }
    }

    impl SettingsProvider for MockSettingsProvider {
        fn data(&self, py: Python<'_>) -> Py<PyDict> {
            self.data.clone_ref(py)
        }

        async fn reload(&self) -> PyResult<()> {
            tokio::task::spawn_blocking(|| {
                Python::attach(|py| {
                    let _ = PyDict::new(py);
                });
            })
            .await
            .unwrap();
            self.is_loaded.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    impl fmt::Display for MockSettingsProvider {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("TestSettingsProvider")
        }
    }

    #[test]
    fn test_convert_keys_to_snake_case_when_normalizing_keys() {
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

        MockSettingsProvider::normalize_keys(&mut data);

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
        let expected_data = BTreeMap::from([(String::from("connection_string"), None)]);
        let mut data = BTreeMap::from([(String::from("ConnectionString"), None)]);

        MockSettingsProvider::normalize_keys(&mut data);

        assert_eq!(data, expected_data);
    }

    #[test]
    fn test_normalize_loaded_data_keeps_empty_map_unchanged() {
        let mut data: BTreeMap<String, Option<String>> = BTreeMap::new();

        MockSettingsProvider::normalize_keys(&mut data);

        assert!(data.is_empty());
    }

    #[test]
    fn test_return_loaded_data() {
        Python::attach(|py| -> PyResult<()> {
            let data = PyDict::new(py);
            data.set_item("setting", "value")?;
            let settings_provider = MockSettingsProvider::new(py, data.unbind());

            let returned_data = SettingsProvider::data(&settings_provider, py);

            assert_eq!(
                returned_data
                    .bind(py)
                    .get_item("setting")?
                    .unwrap()
                    .extract::<String>()?,
                "value"
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_return_type_name() {
        let settings_provider_mock =
            Python::attach(|py| MockSettingsProvider::new(py, PyDict::new(py).unbind()));

        let type_name = settings_provider_mock.get_type_name();

        assert_eq!(type_name, "MockSettingsProvider");
    }

    #[test]
    fn test_load_settings() {
        Python::initialize();
        let settings_provider =
            Python::attach(|py| MockSettingsProvider::new(py, PyDict::new(py).unbind()));

        Python::attach(|py| settings_provider.load(py)).unwrap();

        assert!(settings_provider.is_loaded.load(Ordering::SeqCst));
    }

    #[test]
    fn test_return_found_value_when_key_exists() {
        let key = "setting_key";
        let expected_value = "setting_value";
        Python::attach(|py| -> PyResult<()> {
            let data = PyDict::new(py);
            data.set_item(key, expected_value)?;
            let settings_provider = MockSettingsProvider::new(py, data.unbind());

            let lookup = settings_provider.try_get(py, key)?;

            match lookup {
                SettingLookup::Found { value: Some(value) } => {
                    assert_eq!(value.bind(py).to_str()?, expected_value);
                }
                _ => panic!("Expected a found setting value"),
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_return_missing_value_when_key_does_not_exist() {
        Python::attach(|py| -> PyResult<()> {
            let settings_provider = MockSettingsProvider::new(py, PyDict::new(py).unbind());

            let lookup = settings_provider.try_get(py, "setting_key")?;

            assert!(matches!(lookup, SettingLookup::Missing()));
            Ok(())
        })
        .unwrap();
    }
}
