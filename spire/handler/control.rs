use std::convert::Infallible;

use crate::Label;

///
///
/// [`ControlFlow`]: std::ops::ControlFlow
#[derive(Debug, Default, Clone)]
pub enum ControlFlow {
    /// Wait
    Wait(),
    /// Repeat
    Repeat(),

    /// Break
    Break(),
    /// Break
    BreakOne(Label),

    /// Continue
    #[default]
    Continue,
}

pub trait IntoControlFlow {
    fn into_control_flow(self) -> ControlFlow;
}

impl IntoControlFlow for () {
    fn into_control_flow(self) -> ControlFlow {
        ControlFlow::Continue
    }
}

impl IntoControlFlow for Infallible {
    fn into_control_flow(self) -> ControlFlow {
        ControlFlow::Continue
    }
}
