use std::sync::Arc;

use pyo3::{
    prelude::*,
    types::{PyList, PyString, PyWeakrefMethods, PyWeakrefReference},
};
use tokio::sync::{Mutex, watch};

#[derive(Debug)]
#[pyclass(frozen)]
pub struct RegisteredModel {
    #[pyo3(get)]
    model_reference: Py<PyWeakrefReference>,

    #[pyo3(get)]
    section_path: Option<Py<PyString>>,

    // This field keeps a returned model alive for the lifetime of its Python-facing snapshot, so it doesn't get garbage collected while it's being processed
    _model: Option<Py<PyAny>>,
}

impl RegisteredModel {
    /// Creates a registry entry that tracks a model without retaining a strong reference to it.
    /// This is used when the model is being tracked by the registry but not yet returned to Python.
    fn new_for_tracking(
        model_reference: Py<PyWeakrefReference>,
        section_path: Option<Py<PyString>>,
    ) -> Self {
        Self {
            model_reference,
            section_path,
            _model: None,
        }
    }

    /// Creates a Python-facing snapshot that retains the tracked model.
    /// This is used when the model is being returned to Python, so that it can be used without the risk of being garbage collected.
    fn new_for_snapshot(
        model_reference: Py<PyWeakrefReference>,
        section_path: Option<Py<PyString>>,
        model: Py<PyAny>,
    ) -> Self {
        Self {
            model_reference,
            section_path,
            _model: Some(model),
        }
    }
}

#[derive(Debug)]
#[pyclass(frozen)]
pub struct ModelRegistry {
    sender: watch::Sender<()>,
    models: Arc<Mutex<Py<PyList>>>,
}

#[pymethods]
impl ModelRegistry {
    #[new]
    pub fn new(py: Python<'_>, reload_models_callback: Py<PyWeakrefReference>) -> Self {
        let (sender, mut receiver) = watch::channel(());

        py.detach(|| {
            let runtime = pyo3_async_runtimes::tokio::get_runtime();

            runtime.spawn(async move {
                while receiver.changed().await.is_ok() {
                    Python::attach(|py| Self::invoke_callback(py, &reload_models_callback));
                }
            });
        });

        Self {
            sender,
            models: Arc::new(Mutex::new(PyList::empty(py).unbind())),
        }
    }

    /// Adds a Pydantic model to the registry
    #[pyo3(signature = (model, section_path=None))]
    pub fn add_model(
        &self,
        py: Python<'_>,
        model: &Bound<'_, PyAny>,
        section_path: Option<Py<PyString>>,
    ) -> PyResult<()> {
        let model = model.clone().unbind();

        py.detach(|| {
            let runtime = pyo3_async_runtimes::tokio::get_runtime();
            let models = runtime.block_on(Arc::clone(&self.models).lock_owned());

            Python::attach(|py| {
                let model_reference = PyWeakrefReference::new(model.bind(py))?.unbind();
                let model = RegisteredModel::new_for_tracking(model_reference, section_path);
                models.bind(py).append(Py::new(py, model)?)?;
                Ok(())
            })
        })
    }

    /// Returns all tracked Pydantic models. It will automatically remove models that have been garbage collected.
    #[pyo3(signature = () -> "list[RegisteredModel]")]
    pub fn models(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        py.detach(|| {
            let runtime = pyo3_async_runtimes::tokio::get_runtime();

            tokio::task::block_in_place(|| {
                let models = runtime.block_on(Arc::clone(&self.models).lock_owned());

                Python::attach(|py| {
                    Self::remove_garbage_collected_models(py, models.clone_ref(py))?;
                    Self::create_registered_models_snapshot(py, models.bind(py))
                })
            })
        })
    }
}

impl ModelRegistry {
    /// Called when the settings provider is reloaded. This will notify all models in the registry to reload their values.
    pub fn on_provider_reload(&self) {
        let _ = self.sender.send(());
    }

    fn invoke_callback(py: Python<'_>, callback: &Py<PyWeakrefReference>) {
        let Ok(callback) = callback.bind(py).call0() else {
            return;
        };

        if callback.is_none() {
            return;
        }

        let _ = callback.call0();
    }

    fn remove_garbage_collected_models(py: Python<'_>, models: Py<PyList>) -> PyResult<()> {
        let models = models.into_bound(py);

        for model_index in (0..models.len()).rev() {
            let model = models.get_item(model_index)?;
            let registered_model = model.cast::<RegisteredModel>()?;

            let is_garbage_collected = registered_model
                .borrow()
                .model_reference
                .bind(py)
                .upgrade()
                .is_none();

            if is_garbage_collected {
                models.del_item(model_index)?;
            }
        }

        Ok(())
    }

    fn create_registered_models_snapshot(
        py: Python<'_>,
        models: &Bound<'_, PyList>,
    ) -> PyResult<Py<PyList>> {
        let registered_models = PyList::empty(py);

        for registered_model in models.iter() {
            let registered_model = registered_model.cast::<RegisteredModel>()?;
            let registered_model = registered_model.borrow();

            let Some(model) = registered_model.model_reference.bind(py).upgrade() else {
                continue;
            };

            let section_path = registered_model
                .section_path
                .as_ref()
                .map(|section_path| section_path.clone_ref(py));

            registered_models.append(Py::new(
                py,
                RegisteredModel::new_for_snapshot(
                    registered_model.model_reference.clone_ref(py),
                    section_path,
                    model.unbind(),
                ),
            )?)?;
        }

        Ok(registered_models.unbind())
    }
}

#[cfg(test)]
mod tests {
    use pyo3::{
        prelude::*,
        types::{PyAnyMethods, PyList, PyModule, PyString, PyWeakrefReference},
    };

    use crate::core::{ModelRegistry, RegisteredModel};

    fn create_registry(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<ModelRegistry> {
        let callback = module.getattr("callback")?;
        let callback_reference = PyWeakrefReference::new(&callback)?.unbind();

        Ok(ModelRegistry::new(py, callback_reference))
    }

    #[test]
    fn test_return_registered_models_snapshot() {
        Python::initialize();

        Python::attach(|py| -> PyResult<()> {
            let module = PyModule::from_code(
                py,
                c"def callback():\n    pass\n\nclass Model:\n    pass\n",
                c"",
                c"",
            )?;
            let registry = create_registry(py, &module)?;
            let model = module.getattr("Model")?.call0()?;

            registry.add_model(py, &model, Some(PyString::new(py, "service").unbind()))?;

            let models = registry.models(py)?;
            assert_eq!(models.bind(py).len(), 1);
            let registered_model = models
                .bind(py)
                .get_item(0)?
                .cast::<RegisteredModel>()?
                .borrow();
            assert!(
                registered_model
                    .model_reference
                    .bind(py)
                    .upgrade()
                    .unwrap()
                    .is(&model)
            );
            assert_eq!(
                registered_model
                    .section_path
                    .as_ref()
                    .unwrap()
                    .bind(py)
                    .to_str()?,
                "service"
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_remove_garbage_collected_models_when_getting_registered_models_snapshot() {
        Python::initialize();

        Python::attach(|py| -> PyResult<()> {
            let module = PyModule::from_code(
                py,
                c"def callback():\n    pass\n\nclass Model:\n    pass\n",
                c"",
                c"",
            )?;
            let registry = create_registry(py, &module)?;

            {
                let model = module.getattr("Model")?.call0()?;
                registry.add_model(py, &model, None)?;
            }

            let models = registry.models(py)?;

            assert_eq!(models.bind(py).len(), 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_not_invoke_callback_when_has_expired() {
        Python::initialize();

        Python::attach(|py| -> PyResult<()> {
            let module = PyModule::from_code(
                py,
                c"import weakref\n\ndef create_expired_callback_reference():\n    def callback():\n        pass\n\n    return weakref.ref(callback)\n",
                c"",
                c"",
            )?;
            let callback_reference = module
                .getattr("create_expired_callback_reference")?
                .call0()?
                .cast::<PyWeakrefReference>()?
                .clone()
                .unbind();

            ModelRegistry::invoke_callback(py, &callback_reference);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_ignore_error_when_resolving_callback_reference() {
        Python::initialize();

        Python::attach(|py| -> PyResult<()> {
            let module = PyModule::from_code(
                py,
                c"import weakref\n\ndef callback():\n    pass\n\nclass FailingCallbackReference(weakref.ref):\n    def __call__(self):\n        raise RuntimeError()\n",
                c"",
                c"",
            )?;
            let callback = module.getattr("callback")?;
            let callback_reference = module
                .getattr("FailingCallbackReference")?
                .call1((&callback,))?
                .cast::<PyWeakrefReference>()?
                .clone()
                .unbind();

            ModelRegistry::invoke_callback(py, &callback_reference);

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_invoke_callback_successfully() {
        Python::initialize();

        Python::attach(|py| -> PyResult<()> {
            let module = PyModule::from_code(
                py,
                c"calls = 0\n\ndef callback():\n    global calls\n    calls += 1\n",
                c"",
                c"",
            )?;
            let callback = module.getattr("callback")?;
            let callback_reference = PyWeakrefReference::new(&callback)?.unbind();

            ModelRegistry::invoke_callback(py, &callback_reference);

            assert_eq!(module.getattr("calls")?.extract::<u8>()?, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_invoke_callback_when_provider_reloads() {
        Python::initialize();

        Python::attach(|py| -> PyResult<()> {
            let module = PyModule::from_code(
                py,
                c"calls = 0\n\ndef callback():\n    global calls\n    calls += 1\n",
                c"",
                c"",
            )?;
            let registry = create_registry(py, &module)?;
            let mut receiver = registry.sender.subscribe();

            registry.on_provider_reload();

            let is_notified = py.detach(|| {
                let runtime = pyo3_async_runtimes::tokio::get_runtime();

                runtime.block_on(receiver.changed()).is_ok()
            });

            assert!(is_notified);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_skip_garbage_collected_model_when_creating_registered_models_snapshot() {
        Python::initialize();

        Python::attach(|py| -> PyResult<()> {
            let module = PyModule::from_code(
                py,
                c"import weakref\n\ndef create_expired_model_reference():\n    class Model:\n        pass\n\n    return weakref.ref(Model())\n",
                c"",
                c"",
            )?;
            let model_reference = module
                .getattr("create_expired_model_reference")?
                .call0()?
                .cast::<PyWeakrefReference>()?
                .clone()
                .unbind();
            let models = PyList::empty(py);
            models.append(Py::new(
                py,
                RegisteredModel::new_for_tracking(model_reference, None),
            )?)?;

            let registered_models = ModelRegistry::create_registered_models_snapshot(py, &models)?;

            assert_eq!(registered_models.bind(py).len(), 0);
            Ok(())
        })
        .unwrap();
    }
}
