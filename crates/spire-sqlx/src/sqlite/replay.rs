use std::sync::{Arc, Weak};

use serde::{Deserialize, Serialize};
use spire_core::backend::replay::{ResponseRecorder, ResponseReplay};
use spire_core::http::http::{HeaderName, HeaderValue, StatusCode, Version};
use spire_core::http::{Response, ResponseExt};
use spire_core::{Error, Result};
use sqlx::{Sqlite, SqlitePool, Transaction};

use super::frontier::Inner;
use crate::error::frontier_error;

#[derive(Serialize, Deserialize)]
struct Head {
    status: u16,
    version: u8,
    headers: Vec<(String, Vec<u8>)>,
    uri: Option<String>,
}

struct Recorder {
    pool: SqlitePool,
    id: i64,
    owner: Weak<Inner>,
}

impl ResponseRecorder for Recorder {
    async fn record(&self, response: Response) -> Result<()> {
        let _ownership = self
            .owner
            .upgrade()
            .ok_or_else(|| frontier_error("response belongs to a closed frontier session"))?;
        let version = match response.version() {
            Version::HTTP_09 => 9,
            Version::HTTP_10 => 10,
            Version::HTTP_11 => 11,
            Version::HTTP_2 => 20,
            Version::HTTP_3 => 30,
            _ => return Err(Error::invalid("unsupported response HTTP version")),
        };
        let head = Head {
            status: response.status().as_u16(),
            version,
            headers: response
                .headers()
                .iter()
                .map(|(name, value)| (name.to_string(), value.as_bytes().to_vec()))
                .collect(),
            uri: response.final_uri().map(ToString::to_string),
        };
        let saved = sqlx::query("INSERT INTO spire_frontier_responses (frontier_id, head, body) SELECT id, $2, $3 FROM spire_frontier_items WHERE id = $1 AND in_flight = 1 ON CONFLICT(frontier_id) DO UPDATE SET head = excluded.head, body = excluded.body")
            .bind(self.id).bind(serde_json::to_string(&head).map_err(frontier_error)?).bind(response.body().as_ref())
            .execute(&self.pool).await.map_err(frontier_error)?;
        if saved.rows_affected() != 1 {
            return Err(frontier_error("response occurrence is no longer in flight"));
        }
        Ok(())
    }
}

pub(super) async fn load(
    transaction: &mut Transaction<'_, Sqlite>,
    pool: &SqlitePool,
    id: i64,
    owner: &Arc<Inner>,
) -> Result<ResponseReplay> {
    let stored: Option<(String, Vec<u8>)> =
        sqlx::query_as("SELECT head, body FROM spire_frontier_responses WHERE frontier_id = $1")
            .bind(id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(frontier_error)?;
    let response = stored.map(|(head, body)| decode(&head, body)).transpose()?;
    Ok(ResponseReplay::new(
        response,
        Recorder {
            pool: pool.clone(),
            id,
            owner: Arc::downgrade(owner),
        },
    ))
}

fn decode(head: &str, body: Vec<u8>) -> Result<Response> {
    let head: Head = serde_json::from_str(head).map_err(frontier_error)?;
    let mut response = Response::new(body.into());
    *response.status_mut() = StatusCode::from_u16(head.status).map_err(frontier_error)?;
    *response.version_mut() = match head.version {
        9 => Version::HTTP_09,
        10 => Version::HTTP_10,
        11 => Version::HTTP_11,
        20 => Version::HTTP_2,
        30 => Version::HTTP_3,
        _ => return Err(Error::invalid("invalid stored response HTTP version")),
    };
    for (name, value) in head.headers {
        response.headers_mut().append(
            HeaderName::from_bytes(name.as_bytes()).map_err(frontier_error)?,
            HeaderValue::from_bytes(&value).map_err(frontier_error)?,
        );
    }
    if let Some(uri) = head.uri {
        response.set_final_uri(uri.parse().map_err(frontier_error)?);
    }
    Ok(response)
}
