use std::time::Duration;
use tokio::sync::{mpsc::{self, Receiver, Sender}, oneshot};
use futures::{stream::SelectAll, StreamExt, FutureExt};
use async_stream::stream;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::future::Future;
use tokio::task::{JoinHandle};
use uuid::Uuid;

#[derive(Clone)]
pub struct Channel<T: Send + 'static> {
    sender: Sender<T>,
    receiver: Arc<tokio::sync::Mutex<Receiver<T>>>,
}

impl<T: Send + 'static> Channel<T> {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Channel {
            sender,
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
        }
    }

    pub async fn send(&self, value: T) {
        let _ = self.sender.send(value).await;
    }

    pub async fn recv(&self) -> Option<T> {
        let mut receiver = self.receiver.lock().await;
        receiver.recv().await
    }

    pub fn into_stream(self) -> impl futures::Stream<Item = T> {
        let (sender, mut receiver) = (self.sender.clone(), self.receiver);
        stream! {
            loop {
                let mut lock = receiver.lock().await;
                if let Some(item) = lock.recv().await {
                    yield item;
                } else {
                    break;
                }
            }
        }
    }
}

pub async fn select_channels<T: Send + Unpin + 'static>(channels: Vec<Channel<T>>) -> impl futures::Stream<Item = T> {
    let mut streams = SelectAll::new();
    for ch in channels.into_iter() {
        streams.push(Box::pin(ch.into_stream()));
    }
    streams
}

pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

pub struct TaskManager {
    tasks: Arc<Mutex<HashMap<Uuid, (JoinHandle<()>, oneshot::Sender<()>)>>>,
}

impl TaskManager {
    pub fn new() -> Self {
        TaskManager {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn spawn<F>(&self, fut: F) -> Uuid
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let cancelable = async move {
            tokio::select! {
                _ = fut => {},
                _ = rx => {},
            }
        };

        let handle = tokio::spawn(cancelable);
        let id = Uuid::new_v4();
        self.tasks.lock().unwrap().insert(id, (handle, tx));
        id
    }

    pub fn cancel(&self, id: &Uuid) {
        if let Some((_, tx)) = self.tasks.lock().unwrap().remove(id) {
            let _ = tx.send(());
        }
    }

    pub fn restart<F>(&self, id: &Uuid, new_fut: F) -> Uuid
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.cancel(id);
        self.spawn(new_fut)
    }
}
