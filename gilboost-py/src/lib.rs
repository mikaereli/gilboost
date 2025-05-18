use gilboost_core::{run_main, sleep_ms, Channel};
use pyo3::prelude::*;
use pyo3::types::PyFunction;
use tokio::runtime::Runtime;

#[pyclass]
struct PyChannel {
    inner: Channel<PyObject>,
}

#[pymethods]
impl PyChannel {
    #[new]
    fn new(capacity: usize) -> Self {
        PyChannel {
            inner: Channel::new(capacity),
        }
    }

    fn send(&self, py: Python, msg: PyObject) {
        let inner = self.inner.clone();
        pyo3_asyncio::tokio::run(py, async move {
            inner.send(msg).await;
            Ok(())
        })
        .unwrap();
    }

    fn recv<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        let inner = self.inner.clone();
        pyo3_asyncio::tokio::into_coroutine(py, async move {
            match inner.recv().await {
                Some(val) => Ok(val),
                None => Err(pyo3::exceptions::PyRuntimeError::new_err("Channel closed")),
            }
        })
    }
}

#[pyfunction]
fn spawn(py: Python, func: PyObject) {
    pyo3_asyncio::tokio::run(py, async move {
        tokio::spawn(async move {
            Python::with_gil(|py| {
                func.call0(py).unwrap();
            });
        });
        Ok(())
    })
    .unwrap();
}

#[pyfunction]
fn sleep(py: Python, ms: u64) {
    pyo3_asyncio::tokio::run(py, async move {
        sleep_ms(ms).await;
        Ok(())
    })
    .unwrap();
}

#[pyfunction]
fn run(py: Python, func: PyObject) {
    let rt = Runtime::new().unwrap();
    rt.block_on(async move {
        tokio::spawn(async move {
            Python::with_gil(|py| {
                func.call0(py).unwrap();
            });
        })
        .await
        .unwrap();
    });
}

#[pymodule]
fn gilboost(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(spawn, m)?)?;
    m.add_function(wrap_pyfunction!(sleep, m)?)?;
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_class::<PyChannel>()?;
    Ok(())
}