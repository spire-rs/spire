use std::convert::Infallible;

///
///
/// [`ControlFlow`]: std::ops::ControlFlow
#[derive(Debug, Default, Clone)]
pub enum ControlFlow {
    /// Break
    Break,

    /// Continue
    #[default]
    Continue,
}

pub trait IntoFlow {
    fn into_flow(self) -> ControlFlow;
}

impl IntoFlow for () {
    fn into_flow(self) -> ControlFlow {
        ControlFlow::default()
    }
}

impl IntoFlow for Infallible {
    fn into_flow(self) -> ControlFlow {
        ControlFlow::default()
    }
}
