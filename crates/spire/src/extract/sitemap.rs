use std::io::{BufRead, BufReader};

use derive_more::{Deref, DerefMut};
use flate2::bufread::GzDecoder;
use sitemapo::{Parser, Record};

use crate::backend::Client;
use crate::extract::{Bytes, FromContext};
use crate::{Context, Error, ErrorKind, Result};

/// The magic bytes every gzip stream starts with.
const GZIP: [u8; 2] = [0x1f, 0x8b];

/// Parses the response body as a sitemap: an XML `urlset` or sitemap index,
/// or a text sitemap with one URL per line, gzipped or not.
///
/// Pages are [`Record::Entry`]s and the sitemaps of an index are
/// [`Record::Sitemap`]s, which a handler typically queues in turn.
/// [`FollowSitemap`](crate::FollowSitemap) does exactly that, so most crawls
/// need no handler of their own.
///
/// A sitemap is parsed off the async threads, and capped by the protocol's
/// limits of 50 000 URLs and 50 MiB: the records before a limit are kept.
///
/// ```
/// use spire::extract::Sitemap;
/// use spire::extract::sitemapo::Record;
/// use spire::frontier::queue::Queue;
///
/// async fn sitemap(queue: Queue, Sitemap(records): Sitemap) -> spire::Result<()> {
///     for record in records {
///         if let Record::Entry(entry) = record {
///             queue.follow(entry.location.as_str()).await?;
///         }
///     }
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Sitemap(pub Vec<Record>);

impl<C, S> FromContext<C, S> for Sitemap
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self> {
        let uri = cx.request().uri().clone();
        let Bytes(body) = Bytes::from_context(cx, state).await?;
        let parsed = tokio::task::spawn_blocking(move || parse(&body)).await;
        let records = parsed
            .map_err(|error| Error::worker("sitemap parser panicked").with_source(error))?
            .map_err(|error| error.with_uri(uri))?;
        Ok(Self(records))
    }
}

/// Parses a sitemap, gunzipping it first if needed.
fn parse(body: &[u8]) -> Result<Vec<Record>> {
    if body.starts_with(&GZIP) {
        collect(Parser::new(BufReader::new(GzDecoder::new(body))))
    } else {
        collect(Parser::new(body))
    }
}

/// Collects the records, keeping those before a limit.
fn collect<R: BufRead>(mut parser: Parser<R>) -> Result<Vec<Record>> {
    let mut records = Vec::new();
    for record in parser.by_ref() {
        match record {
            Ok(record) => records.push(record),
            Err(error @ (sitemapo::Error::RecordLimit | sitemapo::Error::ByteLimit)) => {
                tracing::warn!(target: crate::TRACING_TARGET, %error, kept = records.len(), "sitemap over its limit, truncated");
                break;
            }
            Err(error) => return Err(invalid(error.to_string())),
        }
    }

    // An HTML page, e.g. a browser's rendering of the XML, has no sitemap
    // root element, so its format stays undetected.
    if parser.format().is_none() && records.is_empty() {
        return Err(invalid(
            "the response is not a sitemap; fetch sitemaps with an HTTP backend",
        ));
    }
    Ok(records)
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::Rejected).with_message(format!("invalid sitemap: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    const URLSET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <url><loc>https://example.com/a</loc><priority>0.9</priority></url>
          <url><loc>https://example.com/b</loc></url>
        </urlset>"#;

    fn locations(records: &[Record]) -> Vec<String> {
        records
            .iter()
            .map(|record| match record {
                Record::Entry(entry) => entry.location.to_string(),
                Record::Sitemap(sitemap) => format!("sitemap {}", sitemap.location),
            })
            .collect()
    }

    #[test]
    fn parses_xml_text_and_indexes() {
        let urlset = parse(URLSET.as_bytes()).unwrap();
        assert_eq!(
            locations(&urlset),
            ["https://example.com/a", "https://example.com/b"]
        );

        let text = parse(b"https://example.com/a\n\nhttps://example.com/b\n").unwrap();
        assert_eq!(
            locations(&text),
            ["https://example.com/a", "https://example.com/b"]
        );

        let index = r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <sitemap><loc>https://example.com/pages.xml</loc></sitemap>
        </sitemapindex>"#;
        let index = parse(index.as_bytes()).unwrap();
        assert_eq!(locations(&index), ["sitemap https://example.com/pages.xml"]);
    }

    #[test]
    fn gunzips_compressed_sitemaps() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(URLSET.as_bytes()).unwrap();
        let gzipped = encoder.finish().unwrap();

        let records = parse(&gzipped).unwrap();
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn rejects_html_pages() {
        let html = "<!DOCTYPE html><html><body><p>not a sitemap</p></body></html>";
        let error = parse(html.as_bytes()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Rejected);

        // An empty `urlset` is a valid, empty sitemap.
        let empty = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></urlset>"#;
        assert_eq!(parse(empty.as_bytes()).unwrap().len(), 0);
    }
}
