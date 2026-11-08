//! Streams that stop their background task when the last handle is dropped.

use tokio::sync::{mpsc, watch};

pub struct CancelOnDrop<T> {
    updates: mpsc::UnboundedReceiver<T>,
    cancel: watch::Sender<bool>,
}

impl<T> CancelOnDrop<T> {
    pub fn new(updates: mpsc::UnboundedReceiver<T>, cancel: watch::Sender<bool>) -> Self {
        Self { updates, cancel }
    }

    pub async fn next(&mut self) -> Option<T> {
        self.updates.recv().await
    }

    pub fn cancel_handle(&self) -> watch::Sender<bool> {
        self.cancel.clone()
    }
}

impl<T> Drop for CancelOnDrop<T> {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}
