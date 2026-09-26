use pyo3::{prelude::*, types::PyString};

#[pyclass(name = "_SettingLookup", frozen)]
pub enum SettingLookup {
    Missing(),
    Found { value: Option<Py<PyString>> },
}
