# gilboost
> Ускорение Python-приложений с Rust-потоками и отключённым GIL.

## 📦 Описание
**Gilboost** — это фреймворк на Rust для асинхронной обработки задач вне Python Global Interpreter Lock (GIL). Он позволяет обрабатывать тяжёлые задачи на стороне Rust, не блокируя основное Python-приложение.

✅ Подходит для:
- FastAPI
- Flask
- Любых WSGI/ASGI-приложений

## 🔑 Основные возможности
- 🚀 **Асинхронная обработка задач** вне GIL
- 📊 **Очереди приоритетов** для управления последовательностью выполнения
- 🔄 **JSON-сериализация** входных и выходных данных
- 🛡️ **Контроль ресурсов** (память, потоки, TTL)
- 📈 **Мониторинг** состояния очередей и производительности

## 🔧 Установка

### Зависимости
1. Установите Rust и Python-зависимости:
```bash
pip install maturin
```

2. Сборка модуля:
```bash
maturin develop --release
```

Или установите через pip (после публикации):
```bash
pip install gilboost
```

## 📁 Структура проекта
```
gilboost/
├── gilboost/            # Rust crate
│   └── src/lib.rs       # Основной код
├── examples/            # Примеры использования
│   ├── fastapi_app.py   # Пример FastAPI
│   └── flask_app.py     # Пример Flask
```

## 🚀 Быстрый старт

### Инициализация
```python
import gilboost

# Инициализация с настройками по умолчанию
gilboost.init_runtime()

# Или с пользовательскими настройками
gilboost.init_runtime(
    worker_threads=4,       # Количество рабочих потоков
    queue_capacity=1000,    # Максимальное количество задач в очереди
    result_ttl_seconds=3600, # Время жизни результатов (в секундах)
    memory_limit_mb=512     # Ограничение памяти на задачу
)
```

### Отправка задачи на выполнение
```python
import json

# Отправка JSON-данных
task_id = gilboost.submit_task({"data": "value", "array": [1, 2, 3]})

# Отправка бинарных данных
with open('image.jpg', 'rb') as f:
    data = f.read()
    task_id = gilboost.submit_task(data)

# Указание приоритета (задачи с большим приоритетом выполняются раньше)
task_id = gilboost.submit_task(data, priority=10)
```

### Получение результата
```python
result = gilboost.get_result(task_id)

if result is None:
    print("Задача еще обрабатывается")
else:
    # Результат может быть словарем (для JSON) или байтами
    if isinstance(result, dict):
        print(f"Результат JSON: {result}")
    else:
        print(f"Результат bytes: {len(result)} байт")
```

### Получение статистики
```python
stats = gilboost.get_stats()
print(f"Задач в очереди: {stats['queue_size']}")
print(f"Количество результатов: {stats['results_count']}")
print(f"Рабочие потоки: {stats['worker_threads']}")
```

### Очистка данных
```python
# Очистка всех задач и результатов
gilboost.clear_all()
```

## 🌐 Интеграция с веб-фреймворками

### FastAPI
```python
from fastapi import FastAPI, HTTPException, Body
import gilboost
import json

app = FastAPI()
gilboost.init_runtime()

@app.post("/tasks")
async def create_task(data: dict = Body(...), priority: int = 0):
    task_id = gilboost.submit_task(data, priority)
    return {"task_id": task_id}

@app.get("/tasks/{task_id}")
async def get_task_result(task_id: str):
    try:
        result = gilboost.get_result(task_id)
        if result is None:
            return {"status": "processing"}
        return {"status": "completed", "result": result}
    except KeyError:
        raise HTTPException(status_code=404, detail="Task not found")

@app.get("/stats")
async def get_system_stats():
    return gilboost.get_stats()
```

### Flask
```python
from flask import Flask, request, jsonify
import gilboost

app = Flask(__name__)
gilboost.init_runtime()

@app.route('/tasks', methods=['POST'])
def create_task():
    data = request.get_json()
    priority = data.pop('priority', 0) if isinstance(data, dict) else 0
    task_id = gilboost.submit_task(data, priority)
    return jsonify({'task_id': task_id}), 202

@app.route('/tasks/<task_id>', methods=['GET'])
def get_task_result(task_id):
    try:
        result = gilboost.get_result(task_id)
        if result is None:
            return jsonify({'status': 'processing'}), 202
        return jsonify({'status': 'completed', 'result': result}), 200
    except KeyError:
        return jsonify({'error': 'Task not found'}), 404

@app.route('/stats', methods=['GET'])
def get_system_stats():
    stats = gilboost.get_stats()
    return jsonify(stats), 200
```

## 🧪 Пример обработки задач

По умолчанию Gilboost поддерживает обработку JSON и бинарных данных. Вы можете настроить обработку под свои нужды, изменив функцию `process_task` в коде Rust.

Пример стандартной обработки:
```rust
fn process_task(data: &[u8]) -> Vec<u8> {
    // Пытаемся обработать как JSON
    if let Ok(mut json_value) = from_slice::<Value>(data) {
        // Обработка JSON
        if let Value::Object(ref mut map) = json_value {
            map.insert("processed".to_string(), Value::Bool(true));
        }
        to_vec(&json_value).unwrap_or_else(|_| data.to_vec())
    } else {
        // Если не JSON, обрабатываем как бинарные данные
        data.iter().map(|&x| x.saturating_add(1)).collect()
    }
}
```

## 📄 Лицензия
MIT License
