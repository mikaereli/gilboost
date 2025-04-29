use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pyo3::wrap_pyfunction;
use tokio::runtime::{Builder, Runtime};
use once_cell::sync::OnceCell;
use std::sync::{Mutex, Arc};
use crossbeam_channel::{bounded, Sender, Receiver};
use std::thread;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;
use serde_json::{Value, from_slice, to_vec};
use priority_queue::PriorityQueue;

#[derive(Clone)]
struct Config {
    worker_threads: usize,
    queue_capacity: usize,
    result_ttl: Duration,
    memory_limit_mb: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            worker_threads: 8,
            queue_capacity: 1000,
            result_ttl: Duration::from_secs(3600), // 1 час
            memory_limit_mb: 1024, // 1 ГБ
        }
    }
}

struct Task {
    id: Uuid,
    data: Vec<u8>,
    priority: i32,
    created_at: Instant,
}

struct TaskResult {
    data: Vec<u8>,
    created_at: Instant,
}

// Глобальные переменные
static RUNTIME: OnceCell<Mutex<Runtime>> = OnceCell::new();
static CONFIG: OnceCell<Mutex<Config>> = OnceCell::new();
static TASK_QUEUE: OnceCell<Arc<Mutex<PriorityQueue<Uuid, i32>>>> = OnceCell::new();
static TASK_DATA: OnceCell<Arc<Mutex<HashMap<Uuid, Task>>>> = OnceCell::new();
static RESULTS: OnceCell<Arc<Mutex<HashMap<Uuid, TaskResult>>>> = OnceCell::new();
static WORKER_SENDER: OnceCell<Arc<Sender<()>>> = OnceCell::new();

#[pyfunction]
fn init_runtime(
    py: Python,
    worker_threads: Option<usize>,
    queue_capacity: Option<usize>,
    result_ttl_seconds: Option<u64>,
    memory_limit_mb: Option<usize>,
) -> PyResult<()> {
    py.allow_threads(|| {
        if RUNTIME.get().is_some() {
            return Ok(());
        }

        let mut config = Config::default();
        if let Some(wt) = worker_threads {
            config.worker_threads = wt;
        }
        if let Some(qc) = queue_capacity {
            config.queue_capacity = qc;
        }
        if let Some(ttl) = result_ttl_seconds {
            config.result_ttl = Duration::from_secs(ttl);
        }
        if let Some(mem) = memory_limit_mb {
            config.memory_limit_mb = mem;
        }

        CONFIG.set(Mutex::new(config.clone())).map_err(|_| {
            pyo3::exceptions::PyRuntimeError::new_err("Failed to initialize config")
        })?;

        let rt = Builder::new_multi_thread()
            .worker_threads(config.worker_threads)
            .enable_all()
            .build()
            .map_err(|e| {
                pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to create runtime: {}", e))
            })?;

        RUNTIME.set(Mutex::new(rt)).map_err(|_| {
            pyo3::exceptions::PyRuntimeError::new_err("Failed to initialize runtime")
        })?;

        TASK_QUEUE.set(Arc::new(Mutex::new(PriorityQueue::new()))).map_err(|_| {
            pyo3::exceptions::PyRuntimeError::new_err("Failed to initialize task queue")
        })?;

        TASK_DATA.set(Arc::new(Mutex::new(HashMap::new()))).map_err(|_| {
            pyo3::exceptions::PyRuntimeError::new_err("Failed to initialize task data")
        })?;

        RESULTS.set(Arc::new(Mutex::new(HashMap::new()))).map_err(|_| {
            pyo3::exceptions::PyRuntimeError::new_err("Failed to initialize results")
        })?;

        let (tx, rx) = bounded::<()>(1);
        WORKER_SENDER.set(Arc::new(tx)).map_err(|_| {
            pyo3::exceptions::PyRuntimeError::new_err("Failed to initialize worker channel")
        })?;

        let task_queue = TASK_QUEUE.get().unwrap().clone();
        let task_data = TASK_DATA.get().unwrap().clone();
        let results = RESULTS.get().unwrap().clone();

        thread::spawn(move || {
            for _ in 0..config.worker_threads {
                let task_queue = task_queue.clone();
                let task_data = task_data.clone();
                let results = results.clone();
                let rx = rx.clone();

                thread::spawn(move || {
                    loop {
                        let _ = rx.recv_timeout(Duration::from_secs(1));

                        let task_option = {
                            let mut queue = task_queue.lock().unwrap();
                            queue.pop()
                        };

                        if let Some((task_id, _priority)) = task_option {
                            let task = {
                                let mut tasks = task_data.lock().unwrap();
                                tasks.remove(&task_id)
                            };

                            if let Some(task) = task {

                                let result = process_task(&task.data);

                                // Сохраняем результат
                                let mut results_map = results.lock().unwrap();
                                results_map.insert(task.id, TaskResult {
                                    data: result,
                                    created_at: Instant::now(),
                                });
                            }
                        }

                        cleanup_old_results(&results, config.result_ttl);
                    }
                });
            }
        });

        Ok(())
    })
}

// Функция для очистки старых результатов
fn cleanup_old_results(results: &Arc<Mutex<HashMap<Uuid, TaskResult>>>, ttl: Duration) {
    let now = Instant::now();
    let mut results = results.lock().unwrap();

    results.retain(|_, result| now.duration_since(result.created_at) < ttl);
}

fn process_task(data: &[u8]) -> Vec<u8> {
    if let Ok(mut json_value) = from_slice::<Value>(data) {
        if let Value::Object(ref mut map) = json_value {
            map.insert("processed".to_string(), Value::Bool(true));
        }
        to_vec(&json_value).unwrap_or_else(|_| data.to_vec())
    } else {
        data.iter().map(|&x| x.saturating_add(1)).collect()
    }
}

#[pyfunction]
fn submit_task(py: Python, data: &PyAny, priority: Option<i32>) -> PyResult<String> {
    if RUNTIME.get().is_none() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "Runtime not initialized. Call init_runtime() first."
        ));
    }

    let config = CONFIG.get().unwrap().lock().unwrap().clone();

    {
        let task_queue = TASK_QUEUE.get().unwrap().lock().unwrap();
        if task_queue.len() >= config.queue_capacity {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Task queue is full, try again later"
            ));
        }
    }

    let data_bytes = if let Ok(bytes) = data.extract::<&PyBytes>() {
        bytes.as_bytes().to_vec()
    } else if let Ok(dict) = data.extract::<&PyDict>() {
        let dict_str = dict.str()?.to_str()?;
        match serde_json::from_str::<Value>(dict_str) {
            Ok(json_value) => serde_json::to_vec(&json_value)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("JSON serialization error: {}", e)))?,
            Err(e) => return Err(pyo3::exceptions::PyValueError::new_err(format!("Invalid JSON: {}", e))),
        }
    } else {
        return Err(pyo3::exceptions::PyTypeError::new_err("Expected bytes or dict"));
    };

    if data_bytes.len() > config.memory_limit_mb * 1024 * 1024 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            format!("Data size exceeds memory limit of {} MB", config.memory_limit_mb)
        ));
    }

    let task_id = Uuid::new_v4();
    let priority_value = priority.unwrap_or(0);

    {
        let mut task_data = TASK_DATA.get().unwrap().lock().unwrap();
        task_data.insert(task_id, Task {
            id: task_id,
            data: data_bytes,
            priority: priority_value,
            created_at: Instant::now(),
        });
    }

    {
        let mut task_queue = TASK_QUEUE.get().unwrap().lock().unwrap();
        task_queue.push(task_id, -priority_value);
    }

    if let Some(sender) = WORKER_SENDER.get() {
        let _ = sender.try_send(());
    }

    Ok(task_id.to_string())
}

#[pyfunction]
fn get_result(py: Python, task_id: &str) -> PyResult<Option<PyObject>> {
    if RUNTIME.get().is_none() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "Runtime not initialized. Call init_runtime() first."
        ));
    }

    let uuid = Uuid::parse_str(task_id).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!("Invalid UUID: {}", e))
    })?;

    if let Some(results) = RESULTS.get() {
        let results_map = results.lock().unwrap();
        if let Some(result) = results_map.get(&uuid) {
            if let Ok(json) = from_slice::<Value>(&result.data) {
                let py_dict = PyDict::new(py);
                match json {
                    Value::Object(map) => {
                        for (k, v) in map {
                            let py_value = match v {
                                Value::String(s) => s.into_py(py),
                                Value::Number(n) => {
                                    if n.is_i64() {
                                        n.as_i64().unwrap().into_py(py)
                                    } else if n.is_u64() {
                                        n.as_u64().unwrap().into_py(py)
                                    } else {
                                        n.as_f64().unwrap().into_py(py)
                                    }
                                },
                                Value::Bool(b) => b.into_py(py),
                                Value::Null => py.None(),
                                _ => continue,
                            };
                            py_dict.set_item(k, py_value)?;
                        }
                        return Ok(Some(py_dict.into()));
                    },
                    _ => {
                        let py_bytes = PyBytes::new(py, &result.data);
                        return Ok(Some(py_bytes.into()));
                    }
                }
            } else {
                let py_bytes = PyBytes::new(py, &result.data);
                return Ok(Some(py_bytes.into()));
            }
        }
    }

    if let Some(task_data) = TASK_DATA.get() {
        let task_map = task_data.lock().unwrap();
        if task_map.contains_key(&uuid) {
            return Ok(None);
        }
    }

    Err(pyo3::exceptions::PyKeyError::new_err(format!("Task {} not found", task_id)))
}

#[pyfunction]
fn get_stats(py: Python) -> PyResult<PyObject> {
    if RUNTIME.get().is_none() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "Runtime not initialized. Call init_runtime() first."
        ));
    }

    let dict = PyDict::new(py);

    let queue_size = if let Some(task_queue) = TASK_QUEUE.get() {
        task_queue.lock().unwrap().len()
    } else {
        0
    };

    dict.set_item("queue_size", queue_size)?;

    let results_count = if let Some(results) = RESULTS.get() {
        results.lock().unwrap().len()
    } else {
        0
    };

    dict.set_item("results_count", results_count)?;

    if let Some(config) = CONFIG.get() {
        let config = config.lock().unwrap();
        dict.set_item("worker_threads", config.worker_threads)?;
        dict.set_item("queue_capacity", config.queue_capacity)?;
        dict.set_item("result_ttl_seconds", config.result_ttl.as_secs())?;
        dict.set_item("memory_limit_mb", config.memory_limit_mb)?;
    }

    Ok(dict.into())
}

#[pyfunction]
fn clear_all() -> PyResult<()> {
    if RUNTIME.get().is_none() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "Runtime not initialized. Call init_runtime() first."
        ));
    }

    if let Some(task_queue) = TASK_QUEUE.get() {
        task_queue.lock().unwrap().clear();
    }

    if let Some(task_data) = TASK_DATA.get() {
        task_data.lock().unwrap().clear();
    }

    if let Some(results) = RESULTS.get() {
        results.lock().unwrap().clear();
    }

    Ok(())
}


#[pymodule]
fn gilboost(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(init_runtime, m)?)?;
    m.add_function(wrap_pyfunction!(submit_task, m)?)?;
    m.add_function(wrap_pyfunction!(get_result, m)?)?;
    m.add_function(wrap_pyfunction!(get_stats, m)?)?;
    m.add_function(wrap_pyfunction!(clear_all, m)?)?;

    // Добавляем информацию о версии
    m.add("__version__", "0.1.0")?;

    Ok(())
}