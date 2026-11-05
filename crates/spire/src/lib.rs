#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// The README is the crate's documentation. Its examples need several
// features, so they are doctested only with `full` and `testing`, through
// `ReadmeDoctests`.
#![cfg_attr(not(doctest), doc = include_str!("../README.md"))]

/// Model providers, extraction, budgets and caches.
#[cfg(feature = "agent")]
#[cfg_attr(docsrs, doc(cfg(feature = "agent")))]
pub mod agent {
    #[cfg(feature = "rig")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rig")))]
    pub use spire_agent::Rig;
    #[cfg(feature = "rig")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rig")))]
    #[doc(no_inline)]
    pub use spire_agent::rig;
    pub use spire_agent::{
        Agent, AgentCrawlerExt, Budget, Error, Provider, ProviderRequest, ProviderResponse, Result,
        Usage, UsageTotals, cache, objective,
    };
    #[doc(no_inline)]
    pub use spire_agent::{schemars, serde, serde_json};
}

pub mod extract;
pub mod handler;
mod routing;

use spire_core::backend::Backend;
/// Offline helpers for testing handlers, crawls and model providers.
#[cfg(any(test, feature = "testing"))]
#[cfg_attr(docsrs, doc(cfg(feature = "testing")))]
pub mod testing {
    #[cfg(all(feature = "agent", feature = "testing"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "agent", feature = "testing"))))]
    pub use spire_agent::testing::Script;
    pub use spire_core::testing::{MockPage, MockSession, MockSite, Sent, Visit};
}
pub use spire_core::{BoxError, Context, Error, ErrorKind, Result};

pub use crate::routing::Router;

/// Backends and the clients they hand out, plus the enabled HTTP and browser
/// backends.
pub mod backend {
    pub use spire_core::backend::{Backend, Noop, Session, StatusPolicy, document};
    #[cfg(feature = "reqwest")]
    #[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
    #[doc(no_inline)]
    pub use spire_reqwest::reqwest;
    #[cfg(feature = "reqwest")]
    #[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
    pub use spire_reqwest::{ReqwestBackend, ReqwestSession};
    #[cfg(feature = "thirtyfour")]
    #[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
    #[doc(no_inline)]
    pub use spire_thirtyfour::thirtyfour;
    #[cfg(feature = "thirtyfour")]
    #[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
    pub use spire_thirtyfour::{ThirtyfourBackend, ThirtyfourBuilder, ThirtyfourSession};
}

/// Crawl metadata, settings and statistics; the [`Crawler`] itself is at the
/// crate root.
///
/// [`Crawler`]: crate::Crawler
pub mod crawler {
    #[cfg(feature = "robots")]
    #[cfg_attr(docsrs, doc(cfg(feature = "robots")))]
    pub use spire_core::crawler::Robots;
    pub use spire_core::crawler::{
        Attempt, Depth, HostConcurrency, HostDelay, Revisit, Stats, Tag, Tags, event,
    };
    #[cfg(feature = "macros")]
    #[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
    pub use spire_macros::{Revisit, Tags};
}

/// Datasets: where handlers store what they scrape.
pub mod dataset {
    // Listed by name, so the documentation shows the features of this crate.
    #[cfg(feature = "csv")]
    #[cfg_attr(docsrs, doc(cfg(feature = "csv")))]
    pub use spire_core::dataset::CsvDataset;
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub use spire_core::dataset::JsonlDataset;
    pub use spire_core::dataset::{Dataset, InMemDataset};
    #[cfg(feature = "redb")]
    #[cfg_attr(docsrs, doc(cfg(feature = "redb")))]
    pub use spire_redb::RedbDataset;
    #[cfg(feature = "sqlite")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
    pub use spire_sqlx::sqlite::SqliteDataset;
}

/// The frontier: requests waiting to be crawled.
pub mod frontier {
    pub use spire_core::frontier::{
        Ack, Admission, Frontier, FrontierExt, FrontierLayer, InMemFrontier, NextSeed, Report,
        Schedule, facade, queue, score, seed,
    };
    #[cfg(feature = "redb")]
    #[cfg_attr(docsrs, doc(cfg(feature = "redb")))]
    pub use spire_redb::RedbFrontier;
    #[cfg(feature = "sqlite")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
    pub use spire_sqlx::sqlite::SqliteFrontier;

    pub mod seen {
        //! Seen sets: what a [`Dedup`](crate::frontier::facade::Dedup) frontier
        //! remembers.

        pub use spire_core::frontier::seen::{BloomSeen, BloomSnapshot, InMemSeen, SeenSet};
        #[cfg(feature = "redb")]
        #[cfg_attr(docsrs, doc(cfg(feature = "redb")))]
        pub use spire_redb::RedbSeen;
        #[cfg(feature = "sqlite")]
        #[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
        pub use spire_sqlx::sqlite::SqliteSeen;
    }
}

/// Identities: who a request appears to come from, with its proxy,
/// headers and cookies.
pub mod identity {
    #[cfg(feature = "user-agents")]
    #[cfg_attr(docsrs, doc(cfg(feature = "user-agents")))]
    #[doc(inline)]
    pub use spire_core::identity::{Browser, Platform, Profile, UserAgents};
    pub use spire_core::identity::{Identities, Identity, WeakIdentity};
}

/// Stores: one database holding a whole crawl, and the database crates they
/// are built on.
///
/// A store hands out the services it holds, each exported beside the trait it
/// implements: in [`dataset`], [`frontier`], [`frontier::seen`] and
/// [`worker::middleware`].
#[cfg(any(feature = "redb", feature = "sqlite"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "redb", feature = "sqlite"))))]
pub mod store {
    #[cfg(feature = "redb")]
    #[cfg_attr(docsrs, doc(cfg(feature = "redb")))]
    #[doc(no_inline)]
    pub use spire_redb::redb;
    #[cfg(feature = "redb")]
    #[cfg_attr(docsrs, doc(cfg(feature = "redb")))]
    pub use spire_redb::{RedbStore, RedbStoreBuilder, RedbTxnError};
    #[cfg(feature = "sqlite")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
    pub use spire_sqlx::sqlite::{SqliteStore, SqliteStoreBuilder};
    #[cfg(feature = "sqlite")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
    #[doc(no_inline)]
    pub use spire_sqlx::sqlx;
}

pub mod worker {
    //! Workers, which handle fetched requests, their [`Outcome`]s, and the
    //! [`middleware`].

    pub use spire_core::worker::{FromFn, IntoOutcome, Outcome, Scope, Worker, WorkerExt, from_fn};

    pub mod middleware {
        //! Middleware, which runs around a [`Worker`] for every request.
        //!
        //! A [`Middleware`] receives the [`Context`] before any handler, and
        //! the [`Next`] worker to pass it on to. Before passing it on, it can
        //! change the request, e.g. add headers, or answer without fetching.
        //! Around, it can check the response's head: fetched once, it is shared
        //! with the handlers. After, it can change the [`Outcome`].
        //!
        //! [`WorkerExt::layer`] wraps a worker, such as a router, in
        //! middleware. The middleware added last runs first.
        //!
        //! # Examples
        //!
        //! ```
        //! use std::time::Duration;
        //!
        //! use spire::backend::Noop;
        //! use spire::http::http::HeaderValue;
        //! use spire::http::http::header::USER_AGENT;
        //! use spire::worker::middleware::{self, Next, SetHeader};
        //! use spire::worker::{Outcome, WorkerExt, from_fn};
        //! use spire::{Context, Crawler, Result};
        //!
        //! /// Retries pages that answer with a captcha instead of content.
        //! async fn captcha(cx: Context<Noop>, next: Next<Noop>) -> Result<Outcome> {
        //!     if cx.response().await?.headers().contains_key("x-captcha") {
        //!         return Ok(Outcome::Retry {
        //!             after: Some(Duration::from_secs(60)),
        //!         });
        //!     }
        //!     Ok(next.run(cx).await)
        //! }
        //!
        //! let worker = from_fn(|_: Context<Noop>| async {})
        //!     .layer(middleware::from_fn(captcha))
        //!     .layer(SetHeader::new(
        //!         USER_AGENT,
        //!         HeaderValue::from_static("MyBot/1.0"),
        //!     ));
        //! let crawler = Crawler::new(Noop, worker);
        //! ```
        //!
        //! [`Context`]: crate::Context
        //! [`Outcome`]: crate::worker::Outcome
        //! [`Worker`]: crate::worker::Worker

        pub use spire_core::worker::middleware::{
            FromFn, InMemValidator, Layered, Middleware, Next, PageVersion, Revalidate, SetHeader,
            Validator, WorkerExt, from_fn,
        };
        #[cfg(feature = "redb")]
        #[cfg_attr(docsrs, doc(cfg(feature = "redb")))]
        pub use spire_redb::RedbValidator;
        #[cfg(feature = "sqlite")]
        #[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
        pub use spire_sqlx::sqlite::SqliteValidator;
    }
}

/// HTTP requests and responses, their crawl metadata, and the underlying
/// [`http`](self::http::http) crate.
pub mod http {
    #[doc(no_inline)]
    pub use spire_core::http::http;
    pub use spire_core::http::{Request, RequestExt, Response, ResponseExt, UriExt};
}

/// Crawls requests with a [`Backend`], handling them with a [`Router`] by
/// default.
///
/// [`Backend`]: backend::Backend
pub type Crawler<B, W = Router<<B as Backend>::Session>> = spire_core::crawler::Crawler<B, W>;

/// Helpers for code generated by `#[derive(Select)]` and `#[derive(Tags)]`;
/// not public API.
#[doc(hidden)]
pub mod __private {
    pub use std::future::ready;

    pub use crate::extract::context::tag_of;
    #[cfg(feature = "html")]
    pub use crate::extract::html::select::private::*;
}

/// Tracing target for routing and extraction.
const TRACING_TARGET: &str = "spire";

/// The README's examples, compiled as doctests with every feature enabled.
#[cfg(all(doctest, feature = "full", feature = "testing"))]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;

/// The book's examples, one item per chapter, compiled as doctests with
/// every feature enabled. The book is not published with the crate, so this
/// only builds in the repository.
#[cfg(all(doctest, feature = "full", feature = "testing"))]
mod book {
    macro_rules! chapters {
        ($($name:ident: $file:literal),* $(,)?) => {
            $(
                #[doc = include_str!(concat!("../../../book/src/", $file))]
                struct $name;
            )*
        };
    }

    chapters! {
        Introduction: "introduction.md",
        Installation: "installation.md",
        Features: "features.md",
        FirstCrawl: "first-crawl.md",
        Concepts: "concepts.md",
        Routing: "routing.md",
        Extractors: "extractors.md",
        State: "state.md",
        Html: "html.md",
        ModelExtraction: "model-extraction.md",
        AgentProviders: "agent-providers.md",
        AgentResources: "agent-resources.md",
        Objectives: "objectives.md",
        Datasets: "datasets.md",
        Sqlite: "sqlite.md",
        Links: "links.md",
        BestFirst: "best-first.md",
        Sitemaps: "sitemaps.md",
        AvoidingTraps: "avoiding-traps.md",
        Http: "http.md",
        Browsers: "browsers.md",
        Middleware: "middleware.md",
        Identities: "identities.md",
        Frontier: "frontier.md",
        Errors: "errors.md",
        Politeness: "politeness.md",
        Recrawls: "recrawls.md",
        Redb: "redb.md",
        Observability: "observability.md",
        Events: "events.md",
        Testing: "testing.md",
        CustomBackends: "custom-backends.md",
        CustomStorage: "custom-storage.md",
    }
}
