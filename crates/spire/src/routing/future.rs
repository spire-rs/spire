//! Future types for asynchronous route execution.
//!
//! This module provides the [`RouteFuture`] type, which represents the asynchronous
//! execution of a route handler. It serves as the bridge between Spire's routing
//! system and the tower service ecosystem.
//!
//! # Design
//!
//! The [`RouteFuture`] type encapsulates different ways a route can be executed:
//! - As a tower service future that needs to be polled to completion
//! - As a pre-computed [`FlowControl`] value for immediate resolution
//!
//! This design allows for efficient handling of both complex asynchronous operations
//! and simple synchronous responses within the same type system.
//!
//! # Performance
//!
//! The future type is optimized for the common case of service execution while
//! also supporting immediate resolution for cases where the result is already known.
//! This avoids unnecessary async overhead when it's not needed.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use tower::util::{BoxCloneService, Oneshot};

use crate::context::{Context as Cx, FlowControl};

pin_project! {
    /// Future representing the asynchronous execution of a route handler.
    ///
    /// This future encapsulates the execution of route handlers within Spire's
    /// routing system. It can represent either a service future that needs to
    /// be polled to completion, or a pre-computed result for immediate resolution.
    ///
    /// # Type Parameters
    ///
    /// - `C` - The client/context type that the route operates on
    /// - `E` - The error type that the route can produce
    ///
    /// # Usage
    ///
    /// [`RouteFuture`] is typically created internally by the routing system
    /// and should not be constructed directly by user code. It implements
    /// the standard [`Future`] trait and resolves to `Result<FlowControl, E>`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire::routing::RouteFuture;
    /// use spire::context::FlowControl;
    ///
    /// async fn handle_route_future(future: RouteFuture<(), std::convert::Infallible>) {
    ///     match future.await {
    ///         Ok(FlowControl::Continue) => println!("Route completed successfully"),
    ///         Ok(FlowControl::Skip) => println!("Route was skipped"),
    ///         // Handle other FlowControl variants and errors...
    ///         _ => {}
    ///     }
    /// }
    /// ```
    ///
    /// [`Route`]: crate::routing::Route
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct RouteFuture<C, E> {
        #[pin] kind: RouteFutureKind<C, E>,
    }
}

/// Underlying tower service future type.
///
/// This type alias represents the future returned by calling a boxed, cloneable
/// tower service with a context. It's used internally to abstract over the
/// complex tower service types.
type Fut<C, E> = Oneshot<BoxCloneService<Cx<C>, FlowControl, E>, Cx<C>>;

pin_project! {
    /// Internal enum representing different execution states of a route future.
    ///
    /// This enum allows [`RouteFuture`] to efficiently handle both asynchronous
    /// service execution and immediate results. The projection is used by the
    /// pin_project machinery to enable safe pinning of the inner future.
    ///
    /// # Variants
    ///
    /// - [`Future`](Self::Future) - Contains a tower service future that needs polling
    /// - [`FlowControl`](Self::FlowControl) - Contains a pre-computed result
    ///
    /// # Internal Use
    ///
    /// This type is internal to the routing future implementation and should
    /// not be used directly. The public interface operates through [`RouteFuture`].
    #[project = RouteFutureKindProj]
    enum RouteFutureKind<C, E> {
        /// A tower service future that requires polling to completion.
        ///
        /// This variant is used when the route execution involves actual
        /// asynchronous operations that need to be driven to completion.
        Future {
            #[pin] fut: Fut<C, E>,
        },

        /// A pre-computed flow control result ready for immediate resolution.
        ///
        /// This variant is used for optimization when the route result is
        /// already known and doesn't require async execution. The Option
        /// wrapper ensures the value can only be taken once.
        FlowControl {
            flow_control: Option<FlowControl>,
        },
    }
}

impl<C, E> RouteFuture<C, E> {
    /// Creates a new [`RouteFuture`] from a tower service future.
    ///
    /// This constructor creates a route future that will poll the given
    /// tower service future to completion. The future should represent
    /// the execution of a route handler service.
    ///
    /// # Arguments
    ///
    /// * `fut` - The tower service future to wrap
    ///
    /// # Returns
    ///
    /// A new [`RouteFuture`] that will poll the given future when awaited.
    ///
    /// # Usage
    ///
    /// This method is internal to the routing system and is called when
    /// setting up route execution. User code should not call this directly.
    pub(crate) const fn new(fut: Fut<C, E>) -> Self {
        let kind = RouteFutureKind::Future { fut };
        Self { kind }
    }
}

/// Debug implementation for [`RouteFuture`].
///
/// Provides debug formatting that shows the type name without exposing
/// internal details. This is useful for debugging route execution without
/// cluttering the output with complex internal state.
impl<C, E> fmt::Debug for RouteFuture<C, E> {
    /// Formats the route future for debugging output.
    ///
    /// The debug output shows only the type name and indicates that there
    /// are internal fields without exposing their values. This provides
    /// a clean debug representation while maintaining encapsulation.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteFuture").finish_non_exhaustive()
    }
}

/// [`Future`] implementation for [`RouteFuture`].
///
/// This implementation drives route execution to completion, handling both
/// asynchronous service futures and immediate results. It ensures that
/// route execution integrates seamlessly with the async ecosystem.
impl<C, E> Future for RouteFuture<C, E> {
    /// The output type when the future completes.
    ///
    /// Returns a `Result` containing either:
    /// - `Ok(FlowControl)` - Successful route execution with flow control instruction
    /// - `Err(E)` - An error that occurred during route execution
    type Output = Result<FlowControl, E>;

    /// Polls the route future for completion.
    ///
    /// This method drives the underlying route execution, handling different
    /// execution modes:
    ///
    /// - For service futures, polls the underlying tower service future
    /// - For pre-computed results, immediately returns the stored value
    ///
    /// # Arguments
    ///
    /// * `cx` - The task context for waking up the task when progress can be made
    ///
    /// # Returns
    ///
    /// - `Poll::Ready(Ok(flow_control))` - Route completed successfully
    /// - `Poll::Ready(Err(error))` - Route failed with an error
    /// - `Poll::Pending` - Route is still executing, task will be woken when ready
    ///
    /// # Panics
    ///
    /// Panics if the future is polled after completion when using the
    /// `FlowControl` variant. This indicates
    /// a bug in the polling logic as futures should not be polled after
    /// returning `Poll::Ready`.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let flow_control = match this.kind.project() {
            // Poll the underlying tower service future
            RouteFutureKindProj::Future { fut } => match fut.poll(cx) {
                Poll::Ready(Ok(x)) => x,
                Poll::Ready(Err(x)) => return Poll::Ready(Err(x)),
                Poll::Pending => return Poll::Pending,
            },
            // Return the pre-computed result immediately
            RouteFutureKindProj::FlowControl { flow_control } => flow_control
                .take()
                .expect("future should not be polled after completion"),
        };

        Poll::Ready(Ok(flow_control))
    }
}
