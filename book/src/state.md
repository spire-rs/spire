# State

`Router::with_state` shares a value with every handler of a router, such as
a database pool, an API client or settings. A handler takes it with the
`State` extractor, which clones it, so keep large or shared parts behind an
`Arc`.

## Parts of the state

`FromRef` takes a part of the state instead of all of it, so a handler, or
an extractor of your own, asks only for what it needs:

```rust
use spire::Router;
use spire::extract::{FromRef, State};
# use spire::backend::Noop;

#[derive(Clone)]
struct AppState {
    api_key: String,
}

struct ApiKey(String);

impl FromRef<AppState> for ApiKey {
    fn from_ref(state: &AppState) -> Self {
        Self(state.api_key.clone())
    }
}

async fn page(State(ApiKey(key)): State<ApiKey>) {
    println!("{key}");
}

let state = AppState { api_key: "secret".into() };
let router: Router<Noop, AppState> = Router::with_state(state).route("page", page);
```

## Deriving it

With the `macros` feature, `#[derive(FromRef)]` writes these impls: every
field's type gets one, cloning the field, so each handler takes only the
parts it needs. `#[from_ref(skip)]` leaves a field out, e.g. one of two
fields of the same type, which would otherwise conflict:

```rust
use std::sync::Arc;

use spire::Router;
use spire::extract::{FromRef, State};
# use spire::backend::Noop;

#[derive(Clone)]
struct Db;

#[derive(Clone)]
struct Config {
    user_agent: String,
}

#[derive(Clone, FromRef)]
struct AppState {
    db: Db,
    config: Arc<Config>,
}

async fn page(State(_db): State<Db>, State(config): State<Arc<Config>>) {
    println!("{}", config.user_agent);
}

let state = AppState {
    db: Db,
    config: Arc::new(Config { user_agent: "MyBot/1.0".into() }),
};
let router: Router<Noop, AppState> = Router::with_state(state).route("page", page);
```

## Crawler services

`Crawler::with_extension` registers a service independently of router state.
A handler obtains it with `Extension<T>`, which looks up the concrete type
in the request's HTTP extensions:

```rust
use spire::backend::Noop;
use spire::extract::{Extension, State};
use spire::{Crawler, Router};

async fn page(State(prefix): State<String>, Extension(limit): Extension<usize>) {
    println!("{prefix}: {limit}");
}

let router = Router::with_state(String::from("items")).fallback(page);
let crawler = Crawler::new(Noop, router).with_extension(10_usize);
# let _ = crawler;
```

Services are scoped to that crawler. Each request, including retries and
followed links, receives cloned values; use `Arc<T>` for shared services.
Registering the same type again replaces its value. Middleware can override
an extension for one request without changing the crawler's registration.

Missing extensions produce handler errors at runtime. Use
`Option<Extension<T>>` when a service is optional. Prefer `State<T>` for
application dependencies that should be checked through handler type bounds.
With the `agent` feature, `.with_agent(agent)` registers the agent through
this same mechanism without occupying your router state.
