use spire_core::CollectorContext;

#[derive(Debug, Clone)]
pub struct HandlerContext {
    cx: CollectorContext,
    // Request
    // Response
    // TaskQueue
    // DataQueue xS
}

impl HandlerContext {
    pub fn new(cx: CollectorContext) -> Self {
        todo!()
    }

    pub fn build(&self, req: (), resp: ()) -> Self {
        todo!()
    }
}

impl From<CollectorContext> for HandlerContext {
    fn from(value: CollectorContext) -> Self {
        todo!()
    }
}
