use std::time::Duration;
use tokio::sync::mpsc::{self, Receiver, Sender};
use pyo3::prelude::*;
use pyo3::types::PyAny;

#[derive(Clone)]
pub struct Channel<T: Send + 'static> {
    sender: Sender<T>,
    receiver: std::sync::Arc<tokio::sync::Mutex<Receiver<T>>>,
}

impl<T: Send + 'static> Channel<T> {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Channel {
            sender,
            receiver: std::sync::Arc::new(tokio::sync::Mutex::new(receiver)),
        }
    }

    pub async fn send(&self, value: T) {
        let _ = self.sender.send(value).await;
    }

    pub async fn recv(&self) -> Option<T> {
        let mut receiver = self.receiver.lock().await;
        receiver.recv().await
    }
}

pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[tokio::main(flavor = "multi_thread")]
pub async fn run_main<F>(main_func: F)
where
    F: FnOnce() + Send + 'static,
{
    tokio::spawn(async move {
        main_func();
    })
    .await
    .unwrap();
}
