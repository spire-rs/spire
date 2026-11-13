//! Work awaiting robots.txt or host admission.

use std::ops::Deref;

use tokio::task::Id;

use crate::backend::redirect::RedirectRequest;
use crate::frontier::seed::Seed;

/// A frontier occurrence or a continuation of an already-running request.
/// The continuation owns its reply channel and is never serialized as a seed.
#[derive(Debug)]
pub(crate) enum Pending {
    Seed(Seed),
    Redirect(RedirectRequest),
}

impl Pending {
    pub(crate) fn task(&self) -> Option<Id> {
        match self {
            Self::Seed(_) => None,
            Self::Redirect(redirect) => Some(redirect.task),
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        matches!(self, Self::Redirect(redirect) if redirect.answer.is_closed())
    }

    pub(crate) fn is_redirect(&self) -> bool {
        matches!(self, Self::Redirect(_))
    }
}

impl From<Seed> for Pending {
    fn from(seed: Seed) -> Self {
        Self::Seed(seed)
    }
}

impl Deref for Pending {
    type Target = Seed;

    fn deref(&self) -> &Seed {
        match self {
            Self::Seed(seed) => seed,
            Self::Redirect(redirect) => &redirect.seed,
        }
    }
}
