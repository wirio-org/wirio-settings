use pyo3::{prelude::*, types::PyString};

#[pyclass]
pub enum SettingLookup {
    Missing(),
    Found { value: Option<Py<PyString>> },
}
