# GilBoost

High-performance async framework for Python using Rust + Tokio. 

## Features

- Go-like channels: `PyChannel`
- Async task spawning: `spawn()`
- Blocking runner: `run()`
- Sleep: `sleep(ms)`
- Compatible with **FastAPI** / any Python async runtime


| func        | description                                  |
| ----------- | ----------------------------------------- |
| `spawn()`   | to start async's works  |
| `sleep(ms)` | async `sleep` with `tokio::time`   |
| `run()`     | blocking to start works (in `main()` etc) |
| `PyChannel` | сhannels with `send()` and `recv()`              |
| ✔ FastAPI   |  full compatibility                     |

## Installation

```bash
pip install maturin
cd gilboost-py
maturin develop  # or maturin build && pip install target/wheels/...
```


## Usage with FastAPI

```python
from fastapi import FastAPI
import gilboost as gb

app = FastAPI()
queue = gb.PyChannel(10)

@app.on_event("startup")
def setup():
    def worker():
        while True:
            msg = queue.recv()
            print("Got:", msg)
    gb.spawn(worker)

@app.post("/task")
def send_task(payload: dict):
    queue.send(payload)
    return {"status": "queued"}
```

## License

MIT License

