# GilBoost v0.2.0
High-performance async framework for Python using Rust + Tokio. 

## Features

- Go-like channels: `PyChannel`
- Async task spawning: `spawn()`
- Blocking runner: `run()`
- Sleep: `sleep(ms)`
- Compatible with **FastAPI** / any Python async runtime


| func        | description                                  |
| ----------- | ----------------------------------------- |
| `PyChannel`   | buffered channels with async `send()` / `recv()`  |
| `sleep(ms)` | async `sleep` with `tokio::time`   |
| `TaskManager`     | Cancel, restart, and manage async tasks with UUID tracking |
| `select_channels` | Listen on multiple channels concurrently (like `select!` in Go) |
| `async for` | Experimental support for iterators over channels              |
| ✔ FastAPI   |  full compatibility                     |

## Installation

```bash
pip install maturin
cd gilboost-py
maturin develop  # or maturin build && pip install target/wheels/...
```


## Basic using channels

```python
import asyncio
from gilboost_core import PyChannel, sleep

async def sender(ch):
    await ch.send("ping")
    await sleep(1000)
    await ch.send("pong")

async def receiver(ch):
    for _ in range(2):
        print("Got:", await ch.recv())

async def main():
    ch = PyChannel(10)
    await asyncio.gather(sender(ch), receiver(ch))

asyncio.run(main())
```


## FastAPI example

```python
from fastapi import FastAPI
import gilboost_core as gb

app = FastAPI()
queue = gb.PyChannel(10)

@app.on_event("startup")
def setup():
    async def worker():
        while True:
            msg = await queue.recv()
            print("Got:", msg)
    import asyncio
    asyncio.create_task(worker())

@app.post("/task")
async def send_task(payload: dict):
    await queue.send(str(payload))
    return {"status": "queued"}
```

## TaskManager: Supervision & Cancellation

```python
from gilboost_core import TaskManager, sleep
import asyncio

manager = TaskManager()

async def job():
    while True:
        print("Working...")
        await sleep(500)

async def main():
    task_id = manager.spawn(job())
    await sleep(1500)
    manager.cancel(task_id)

asyncio.run(main())
```

## Preparing for v0.3.0
- Full `async for` support in Python
- Native Rust-type messages (not just strings)
- FastAPI middleware for lifecycle management
- Graceful shutdowns and metrics


## License

MIT License

