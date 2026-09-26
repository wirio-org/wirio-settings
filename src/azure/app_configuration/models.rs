use pyo3::prelude::*;

#[pyclass(name = "_KeyFilter", frozen)]
pub struct KeyFilter;

#[pymethods]
impl KeyFilter {
    #[classattr]
    pub const ANY: &'static str = "*";
}

#[pyclass(name = "_NameFilter", frozen)]
pub struct NameFilter;

#[pymethods]
impl NameFilter {
    #[classattr]
    pub const ANY: &'static str = "*";
}

#[pyclass(name = "_LabelFilter", frozen)]
pub struct LabelFilter;

#[pymethods]
impl LabelFilter {
    #[classattr]
    pub const NULL: &'static str = "\0";
}

#[derive(Debug, Clone)]
#[pyclass(name = "_SettingSelector", frozen, from_py_object)]
pub struct SettingSelector {
    pub key_filter: String,
    pub label_filter: String,
}

#[pymethods]
impl SettingSelector {
    #[new]
    #[pyo3(signature = (key_filter, label_filter=None))]
    /// Creates a setting selector. When `label_filter` is not specified, it loads values without a label.
    pub fn new(key_filter: String, label_filter: Option<String>) -> Self {
        Self {
            key_filter,
            label_filter: label_filter.unwrap_or_else(|| String::from(LabelFilter::NULL)),
        }
    }
}

#[derive(Debug, Clone)]
#[pyclass(name = "_FeatureFlagSelector", frozen, from_py_object)]
pub struct FeatureFlagSelector {
    pub name_filter: String,
    pub label_filter: String,
}

#[pymethods]
impl FeatureFlagSelector {
    #[new]
    #[pyo3(signature = (name_filter, label_filter=None))]
    /// Creates a feature flag selector. When `label_filter` is not specified, it loads values without a label.
    pub fn new(name_filter: String, label_filter: Option<String>) -> Self {
        Self {
            name_filter,
            label_filter: label_filter.unwrap_or_else(|| String::from(LabelFilter::NULL)),
        }
    }
}
