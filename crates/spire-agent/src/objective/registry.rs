use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, MutexGuard};

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use spire_core::http::UriExt;
use spire_core::http::http::Uri;
use tokio::sync::Mutex as AsyncMutex;

use super::{Objective, Settings};
use crate::{Error, Result};

/// Why an objective finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Newly accepted records support the model completion condition.
    Met,
    /// The accepted-record target was reached.
    TargetItems,
    /// The successful relevant-page target was reached.
    TargetPages,
    /// Too many successful pages completed without progress.
    Stalled,
}

/// Accepted evidence and successful page progress for one objective.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Progress {
    /// Distinct successful pages processed while this objective was active.
    pub pages: u64,
    /// Successful pages with accepted records or a relevant incoming rating.
    pub relevant_pages: u64,
    /// Records reported after successful writes; duplicates count again.
    pub items: u64,
    /// Evidence retained for completion judgments, bounded to 128 items and 32 KiB.
    pub record: Vec<Value>,
    /// Records omitted from evidence after its bounds were reached.
    pub omitted: u64,
}

/// Which objective completions stop the worker's crawl.
#[derive(Debug, Default, Clone, Copy)]
pub enum StopPolicy {
    /// Stop when all objectives finish.
    #[default]
    All,
    /// Stop when any objective finishes.
    Any,
}

#[derive(Default)]
pub(super) struct State {
    pub progress: Progress,
    pub finished: Option<StopReason>,
    pub revision: u64,
    pub checked_revision: u64,
    bytes: usize,
    pub visited: HashSet<String>,
    records_on: HashSet<String>,
    last_progress: u64,
    reported_revision: u64,
}

type Encode = fn(&dyn Any) -> Result<Vec<Value>>;
pub(super) struct Entry {
    pub id: TypeId,
    batch: TypeId,
    pub name: &'static str,
    pub settings: Settings,
    pub schema: Value,
    encode: Encode,
    pub state: Mutex<State>,
    pub ratings: AsyncMutex<HashMap<String, i32>>,
    pub judge: AsyncMutex<()>,
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Entry {
    pub fn active(&self) -> bool {
        lock(&self.state).finished.is_none()
    }

    fn limits(&self, state: &mut State) {
        if state.finished.is_some() {
            return;
        }
        state.finished = if self
            .settings
            .items
            .is_some_and(|n| state.progress.items >= n)
        {
            Some(StopReason::TargetItems)
        } else if self
            .settings
            .pages
            .is_some_and(|n| state.progress.relevant_pages >= n)
        {
            Some(StopReason::TargetPages)
        } else if self
            .settings
            .stall
            .is_some_and(|n| state.progress.pages.saturating_sub(state.last_progress) >= n)
        {
            Some(StopReason::Stalled)
        } else {
            None
        };
    }
}

/// Shared objective services inserted automatically by objective middleware.
///
/// Applications normally configure this through [`super::ObjectiveExt`].
/// Cloning this handle shares its storage without copying objective state.
/// Progress belongs to the middleware and survives repeated crawl executions.
#[derive(Clone, Default)]
pub struct ObjectiveRegistry {
    inner: Arc<Inner>,
}

#[derive(Clone, Default)]
struct Inner {
    entries: Vec<Arc<Entry>>,
    stop: StopPolicy,
}

impl ObjectiveRegistry {
    pub(super) fn add<T: Serialize + JsonSchema + 'static>(&mut self, objective: Objective<T>) {
        let entry = Entry {
            id: TypeId::of::<T>(),
            batch: TypeId::of::<Vec<T>>(),
            name: std::any::type_name::<T>(),
            settings: objective.settings,
            schema: schemars::schema_for!(T).to_value(),
            encode: |batch| {
                batch
                    .downcast_ref::<Vec<T>>()
                    .ok_or_else(|| Error::Configuration("objective batch type mismatch".into()))?
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|error| Error::Answer(error.to_string()))
            },
            state: Mutex::default(),
            ratings: AsyncMutex::default(),
            judge: AsyncMutex::new(()),
        };
        entry.limits(&mut lock(&entry.state));
        Arc::make_mut(&mut self.inner).entries.push(Arc::new(entry));
    }

    pub(super) fn entries(&self) -> &[Arc<Entry>] {
        &self.inner.entries
    }

    pub(super) fn set_stop_policy(&mut self, stop: StopPolicy) {
        Arc::make_mut(&mut self.inner).stop = stop;
    }

    pub(super) fn validate(&self) -> Result<()> {
        let mut types = HashSet::new();
        for entry in &self.inner.entries {
            if !types.insert(entry.id) {
                return Err(Error::Configuration(format!(
                    "duplicate objective record type `{}`",
                    entry.name
                )));
            }
        }
        Ok(())
    }

    /// Whether the group stopping condition has been reached.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        match self.inner.stop {
            StopPolicy::All => self.inner.entries.iter().all(|entry| !entry.active()),
            StopPolicy::Any => self.inner.entries.iter().any(|entry| !entry.active()),
        }
    }

    /// Returns progress for a registered record type.
    ///
    /// # Errors
    /// Returns invalid configuration if this type has no registered objective.
    pub fn get<T: 'static>(&self) -> spire_core::Result<ObjectiveStatus<T>> {
        self.inner
            .entries
            .iter()
            .find(|entry| entry.id == TypeId::of::<T>())
            .map(|entry| ObjectiveStatus {
                entry: entry.clone(),
                marker: PhantomData,
            })
            .ok_or_else(|| {
                spire_core::Error::invalid(format!(
                    "no objective registered for `{}`",
                    std::any::type_name::<T>()
                ))
            })
    }

    /// Whether a collector should skip a completed objective before fetching.
    #[must_use]
    pub fn completed<T: 'static>(&self) -> bool {
        self.inner
            .entries
            .iter()
            .any(|entry| entry.id == TypeId::of::<T>() && !entry.active())
    }

    /// Prepares evidence for a registered `Vec<T>` before dataset writes.
    ///
    /// Unregistered batches return `None`. Ordinary collectors need no
    /// `Serialize` bound: encoding is supplied by the typed registration.
    ///
    /// # Errors
    /// Returns serialization or invalid source URL errors before any writes.
    pub fn prepare(&self, batch: &dyn Any, source: &Uri) -> Result<Option<PreparedEvidence>> {
        self.inner
            .entries
            .iter()
            .find(|entry| entry.batch == batch.type_id())
            .map(|entry| {
                let source = canonical(source)?;
                let values = (entry.encode)(batch)?;
                Ok(PreparedEvidence {
                    entry: entry.clone(),
                    source,
                    values,
                })
            })
            .transpose()
    }

    pub(super) async fn page_done(&self, source: &Uri, active: &[TypeId]) -> Result<()> {
        let source = canonical(source)?;
        for entry in &self.inner.entries {
            if !active.contains(&entry.id) {
                continue;
            }
            let relevant = entry
                .ratings
                .lock()
                .await
                .get(&source)
                .is_some_and(|score| *score >= i32::from(entry.settings.threshold));
            let mut state = lock(&entry.state);
            if !state.visited.insert(source.clone()) {
                continue;
            }
            state.progress.pages = state.progress.pages.saturating_add(1);
            if state.records_on.contains(&source) || relevant {
                state.progress.relevant_pages = state.progress.relevant_pages.saturating_add(1);
                state.last_progress = state.progress.pages;
            }
            if state.revision != state.reported_revision {
                state.last_progress = state.progress.pages;
                state.reported_revision = state.revision;
            }
            entry.limits(&mut state);
        }
        Ok(())
    }
}

/// Evidence waiting for the application's successful storage operation.
pub struct PreparedEvidence {
    entry: Arc<Entry>,
    source: String,
    values: Vec<Value>,
}
impl PreparedEvidence {
    /// Accepts a prepared batch after every dataset write succeeds.
    pub fn accept(self) {
        if self.values.is_empty() {
            return;
        }
        let mut state = lock(&self.entry.state);
        state.records_on.insert(self.source);
        state.progress.items = state
            .progress
            .items
            .saturating_add(u64::try_from(self.values.len()).unwrap_or(u64::MAX));
        for value in self.values {
            let bytes = value.to_string().len();
            if state.progress.record.len() < 128 && state.bytes.saturating_add(bytes) <= 32 * 1024 {
                state.bytes += bytes;
                state.progress.record.push(value);
            } else {
                state.progress.omitted = state.progress.omitted.saturating_add(1);
            }
        }
        state.revision = state.revision.saturating_add(1);
        self.entry.limits(&mut state);
    }
}

/// Read-only shared progress for one registered objective.
pub struct ObjectiveStatus<T> {
    entry: Arc<Entry>,
    marker: PhantomData<fn() -> T>,
}
impl<T> ObjectiveStatus<T> {
    /// Returns accepted records and successful page counts.
    #[must_use]
    pub fn progress(&self) -> Progress {
        lock(&self.entry.state).progress.clone()
    }

    /// Returns the first reason this objective finished, or `None` if unmet.
    #[must_use]
    pub fn stop_reason(&self) -> Option<StopReason> {
        lock(&self.entry.state).finished
    }
}
impl<T> Clone for ObjectiveStatus<T> {
    fn clone(&self) -> Self {
        Self {
            entry: self.entry.clone(),
            marker: PhantomData,
        }
    }
}

pub(super) fn canonical(uri: &Uri) -> Result<String> {
    if uri.scheme().is_none() || uri.authority().is_none() {
        return Err(Error::Configuration(format!(
            "objective URI `{uri}` is not absolute"
        )));
    }
    Ok(uri.canonical())
}
