use pyo3::prelude::*;

#[pyclass]
pub enum SettingLookup {
    Missing(),
    Found { value: Option<String> },
}
