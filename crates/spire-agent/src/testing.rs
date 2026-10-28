//! Scripted providers for offline tests and examples.

use std::collections::VecDeque;
use std::fmt;
use std::future::{Future, ready};
use std::sync::{Arc, Mutex, MutexGuard};

use futures::future::BoxFuture;

use crate::{CompletionRequest, CompletionResponse, Error, Provider, Result};

type Handler =
    dyn Fn(CompletionRequest) -> BoxFuture<'static, Result<CompletionResponse>> + Send + Sync;

enum Replies {
    Queue(Mutex<VecDeque<CompletionResponse>>),
    Handler(Box<Handler>),
}

struct State {
    requests: Mutex<Vec<CompletionRequest>>,
    replies: Replies,
}

/// A provider that records requests and returns scripted answers.
///
/// No model or network is used. Clones share their response queue and request
/// log. See [`Self::from_fn`] for request-dependent replies, and
/// [`Self::from_async_fn`] for latency and cancellation tests.
#[derive(Clone)]
pub struct Script {
    state: Arc<State>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Script {
    /// Returns responses in order.
    ///
    /// Calling after the queue is exhausted returns a provider error; it never
    /// repeats an answer implicitly.
    #[must_use]
    pub fn new(responses: impl IntoIterator<Item = CompletionResponse>) -> Self {
        Self {
            state: Arc::new(State {
                requests: Mutex::default(),
                replies: Replies::Queue(Mutex::new(responses.into_iter().collect())),
            }),
        }
    }

    /// Creates replies synchronously from each request.
    ///
    /// An error can be returned to simulate a provider failure.
    #[must_use]
    pub fn from_fn(
        handler: impl Fn(&CompletionRequest) -> Result<CompletionResponse> + Send + Sync + 'static,
    ) -> Self {
        Self::from_async_fn(move |request| ready(handler(&request)))
    }

    /// Creates replies asynchronously.
    ///
    /// No internal lock is held while the callback runs, so calls can complete
    /// in a different order to arrival.
    #[must_use]
    pub fn from_async_fn<F, Fut>(handler: F) -> Self
    where
        F: Fn(CompletionRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<CompletionResponse>> + Send + 'static,
    {
        Self {
            state: Arc::new(State {
                requests: Mutex::default(),
                replies: Replies::Handler(Box::new(move |request| Box::pin(handler(request)))),
            }),
        }
    }

    /// Returns requests in arrival order, including calls whose callback failed
    /// or was cancelled.
    #[must_use]
    pub fn requests(&self) -> Vec<CompletionRequest> {
        lock(&self.state.requests).clone()
    }

    /// Returns the number of provider calls received, excluding agent cache
    /// hits.
    #[must_use]
    pub fn calls(&self) -> usize {
        lock(&self.state.requests).len()
    }
}

impl Default for Script {
    fn default() -> Self {
        Self::new([])
    }
}

impl fmt::Debug for Script {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Script")
            .field("calls", &self.calls())
            .finish_non_exhaustive()
    }
}

impl Provider for Script {
    fn complete(
        &self,
        request: CompletionRequest,
    ) -> impl Future<Output = Result<CompletionResponse>> + Send {
        match &self.state.replies {
            Replies::Queue(queue) => {
                // Capture and select the response under the same ordering lock.
                let mut requests = lock(&self.state.requests);
                requests.push(request);
                let response = lock(queue).pop_front().ok_or_else(|| {
                    Error::provider(std::io::Error::other("scripted provider has no response"))
                });
                drop(requests);
                Box::pin(ready(response)) as BoxFuture<'static, Result<CompletionResponse>>
            }
            Replies::Handler(handler) => {
                lock(&self.state.requests).push(request.clone());
                handler(request)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Usage;

    fn request(input: &str) -> CompletionRequest {
        CompletionRequest {
            system: "test".into(),
            input: input.into(),
            schema: None,
            max_tokens: 10,
        }
    }

    #[tokio::test]
    async fn clones_share_responses_and_capture_failed_calls() {
        let provider = Script::new([CompletionResponse {
            text: "one".into(),
            usage: Usage::tokens(2, 1),
        }]);
        let clone = provider.clone();
        assert_eq!(
            provider.complete(request("first")).await.unwrap().text,
            "one"
        );
        assert!(matches!(
            clone.complete(request("second")).await,
            Err(Error::Provider(_))
        ));
        assert_eq!(provider.calls(), 2);
        assert_eq!(
            clone
                .requests()
                .iter()
                .map(|request| request.input.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        provider.requests().clear();
        assert_eq!(provider.calls(), 2);
    }

    #[tokio::test]
    async fn callbacks_keep_request_fields_and_their_error_category() {
        let provider = Script::from_fn(|request| {
            assert_eq!(request.system, "test");
            assert_eq!(request.max_tokens, 10);
            Err(Error::Budget("scripted failure"))
        });
        assert!(matches!(
            provider.complete(request("input")).await,
            Err(Error::Budget("scripted failure"))
        ));
        assert_eq!(provider.calls(), 1);
    }

    #[test]
    fn concurrent_queued_responses_match_captured_request_order() {
        const THREADS: usize = 16;
        const CALLS: usize = 32;
        let provider = Script::new((0..THREADS * CALLS).map(|index| CompletionResponse {
            text: index.to_string(),
            usage: Usage::tokens(0, 0),
        }));
        let barrier = std::sync::Barrier::new(THREADS);
        let replies = std::thread::scope(|scope| {
            let tasks = (0..THREADS)
                .map(|thread| {
                    let provider = &provider;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        (0..CALLS)
                            .map(|call| {
                                let input = format!("{thread}:{call}");
                                let response =
                                    futures::executor::block_on(provider.complete(request(&input)))
                                        .unwrap();
                                (input, response.text)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            tasks
                .into_iter()
                .flat_map(|task| task.join().unwrap())
                .collect::<std::collections::HashMap<_, _>>()
        });
        let requests = provider.requests();
        assert_eq!(requests.len(), THREADS * CALLS);
        for (index, request) in requests.iter().enumerate() {
            assert_eq!(replies[&request.input], index.to_string());
        }
    }
}
