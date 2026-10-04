use pyo3::{exceptions::PyValueError, prelude::*};

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

#[derive(Debug, Clone, PartialEq, Eq)]
#[pyclass(name = "_SettingSelector", frozen, from_py_object)]
pub struct SettingSelector {
    pub key_filter: Option<String>,
    pub label_filter: Option<String>,
    pub snapshot_name: Option<String>,
}

#[pymethods]
impl SettingSelector {
    #[new]
    #[pyo3(signature = (key_filter=None, label_filter=None, snapshot_name=None))]
    /// Creates a setting selector. When `label_filter` is not specified, it loads values without a label. `snapshot_name` cannot be combined with filters.
    pub fn new(
        key_filter: Option<String>,
        label_filter: Option<String>,
        snapshot_name: Option<String>,
    ) -> PyResult<Self> {
        if snapshot_name.is_some() && (key_filter.is_some() || label_filter.is_some()) {
            return Err(PyValueError::new_err(
                "'snapshot_name' cannot be combined with 'key_filter' or 'label_filter'",
            ));
        }

        if snapshot_name.is_none() && key_filter.is_none() {
            return Err(PyValueError::new_err(
                "'key_filter' or 'snapshot_name' must be specified",
            ));
        }

        Ok(Self {
            key_filter,
            label_filter: snapshot_name
                .is_none()
                .then(|| label_filter.unwrap_or_else(|| String::from(LabelFilter::NULL))),
            snapshot_name,
        })
    }
}

impl Default for SettingSelector {
    fn default() -> Self {
        Self {
            key_filter: Some(String::from(KeyFilter::ANY)),
            label_filter: Some(String::from(LabelFilter::NULL)),
            snapshot_name: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Default for FeatureFlagSelector {
    fn default() -> Self {
        Self {
            name_filter: String::from(NameFilter::ANY),
            label_filter: String::from(LabelFilter::NULL),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyFilter, LabelFilter, SettingSelector};

    #[test]
    fn test_reject_snapshot_name_with_filters() {
        let error = SettingSelector::new(
            Some(String::from("service.*")),
            None,
            Some(String::from("production")),
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "ValueError: 'snapshot_name' cannot be combined with 'key_filter' or 'label_filter'"
        );
    }

    #[test]
    fn test_require_key_filter_without_snapshot_name() {
        let error = SettingSelector::new(None, None, None).unwrap_err();

        assert_eq!(
            error.to_string(),
            "ValueError: 'key_filter' or 'snapshot_name' must be specified"
        );
    }

    #[test]
    fn test_create_default_selector() {
        let selector = SettingSelector::default();

        assert_eq!(selector.key_filter.as_deref(), Some(KeyFilter::ANY));
        assert_eq!(selector.label_filter.as_deref(), Some(LabelFilter::NULL));
        assert_eq!(selector.snapshot_name, None);
    }
}
