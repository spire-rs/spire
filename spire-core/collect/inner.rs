use std::convert::Infallible;
use std::sync::atomic::AtomicBool;

use tokio::sync::mpsc::{Receiver, Sender, unbounded_channel};
use tower_service::Service;

use crate::collect::{Result, Signal};
use crate::collect::{Context, Metrics, Request, Response};

// struct Daemon<B> {
//     rx: Receiver<Request<B>>
// }

pub struct CollectorInner<C, R, B = ()> {
    is_running: AtomicBool,
    task_queue_rx: Receiver<Request<B>>,
    task_queue_tx: Sender<Request<B>>,
    worker: C, // MakeService
    router: R, // MakeService
}

impl<C, R, B> CollectorInner<C, R, B> {
    pub fn new(worker: C, router: R) -> Self {
        let (tx, rx) = unbounded_channel::<Request<B>>();
        // Self {
        //     task_rx: rx,
        //     task_tx: tx,
        //     worker,
        //     router,
        // }
    }

    pub async fn add(&self, task: Request<B>) {
        self.task_queue_tx.send(task).await.expect("should not be closed");
    }
}

impl<C, R, CT, RT, B> CollectorInner<C, R, B>
where
    C: Service<(), Response = CT, Error = Infallible>,
    R: Service<(), Response = RT, Error = Infallible>,
    CT: Service<Request<B>, Response = Response<B>, Error = Signal>,
    RT: Service<Context<B>, Response = Signal, Error = Infallible>,
{
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub async fn run(&self) -> Result<Metrics> {
        todo!()
    }

    pub fn abort(&self) -> Result<()> {
        todo!()
    }
}
