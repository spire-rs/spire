use tokio::sync::mpsc::Sender;

use crate::collect::Request;

pub struct TaskQueue<B> {
    tx: Sender<Request<B>>,
}

impl<B> TaskQueue<B> {
    pub fn new(tx: Sender<Request<B>>) -> Self {
        Self { tx }
    }
}

impl<B> From<Sender<Request<B>>> for TaskQueue<B> {
    fn from(tx: Sender<Request<B>>) -> Self {
        Self::new(tx)
    }
}
