pub use body::Body;
pub use queue::TaskQueue;
pub use task::{Request, Response, Tag, Task};

use crate::backend::Backend;

mod body;
mod queue;
mod task;

/// Framework-specific [`Context`] type.
pub struct Context<B> {
    queue: TaskQueue,
    task: Task,
    backend: B,
}

impl<B> Context<B> {
    pub fn new(backend: B, request: impl Into<Request>) -> Self {
        Self {
            queue: TaskQueue::new(),
            task: Task::new(request),
            backend,
        }
    }

    pub async fn resolve(self)
    where
        B: Backend,
    {
        todo!()
    }

    pub fn tag(&self) -> &Tag {
        todo!()
    }
}
