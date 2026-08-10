use pyo3::{prelude::*, types::PyString};

#[pyclass(frozen)]
pub enum SettingLookup {
    Missing(),
    Found { value: Option<Py<PyString>> },
}
