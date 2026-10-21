use std::future::{Future, ready};

use derive_more::{Deref, DerefMut};
use serde::de::DeserializeOwned;

use crate::extract::FromContext;
use crate::{Context, Error, Result};

/// Extracts the request URI's query string, deserialized as `T`.
///
/// A URI without a query deserializes from an empty one, so a `T` whose
/// fields are all optional or defaulted always extracts. The request is not
/// fetched.
///
/// ```
/// use serde::Deserialize;
/// use spire::extract::Query;
/// use spire::frontier::queue::Queue;
///
/// #[derive(Deserialize)]
/// struct Page {
///     #[serde(default)]
///     page: u32,
/// }
///
/// async fn listing(Query(Page { page }): Query<Page>, queue: Queue) -> spire::Result<()> {
///     queue.follow(format!("?page={}", page + 1)).await?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Default, Clone, Copy, Deref, DerefMut)]
pub struct Query<T>(pub T);

impl<C, S, T> FromContext<C, S> for Query<T>
where
    C: Sync,
    S: Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = Error;

    fn from_context(cx: &Context<C>, _: &S) -> impl Future<Output = Result<Self>> + Send {
        let query = cx.request().uri().query().unwrap_or_default();
        ready(
            serde_urlencoded::from_str(query)
                .map(Self)
                .map_err(|error| Error::worker("failed to parse the query").with_source(error)),
        )
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::backend::Noop;
    use crate::frontier::seed::Seed;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Search {
        q: String,
        #[serde(default)]
        page: u32,
    }

    async fn extract(uri: &str) -> Result<Query<Search>> {
        let cx = Context::new(Seed::get(uri).unwrap().into_request(), Noop);
        Query::<Search>::from_context(&cx, &()).await
    }

    #[tokio::test]
    async fn deserializes_the_query() {
        let Query(search) = extract("https://example.com/?q=lamp%20desk&page=2")
            .await
            .unwrap();
        assert_eq!(
            search,
            Search {
                q: "lamp desk".into(),
                page: 2
            }
        );

        let Query(search) = extract("https://example.com/?q=chair").await.unwrap();
        assert_eq!(search.page, 0);
    }

    #[tokio::test]
    async fn rejects_queries_that_do_not_fit() {
        assert!(extract("https://example.com/").await.is_err());
        assert!(extract("https://example.com/?q=a&page=two").await.is_err());
    }
}
