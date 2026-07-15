//! Oracle metadata tables: the on-chain mirror (`oracle_metadata`) + the hosted
//! extended-JSON store (`oracle_meta_json`).

use anyhow::Result;
use tokio_postgres::Client;

/// Index oracle metadata from a `write_oracle_meta` instruction. The account is
/// write-once on-chain, so keep the first row (idempotent re-processing).
#[allow(clippy::too_many_arguments)]
pub async fn insert_oracle_meta(
    client: &Client,
    oracle: &str,
    subject: &str,
    options: &serde_json::Value,
    uri: &str,
    uri_hash: &str,
    slot: i64,
    signature: &str,
) -> Result<()> {
    client
        .execute(
            "INSERT INTO oracle_metadata (oracle, subject, options, uri, uri_hash, slot, signature)
             VALUES ($1,$2,$3,$4,$5,$6,$7)
             ON CONFLICT (oracle) DO NOTHING",
            &[
                &oracle, &subject, options, &uri, &uri_hash, &slot, &signature,
            ],
        )
        .await?;
    Ok(())
}

fn meta_json(r: &tokio_postgres::Row) -> serde_json::Value {
    serde_json::json!({
        "oracle": r.get::<_, String>(0),
        "subject": r.get::<_, String>(1),
        "options": r.get::<_, serde_json::Value>(2),
        "uri": r.get::<_, String>(3),
        "uriHash": r.get::<_, String>(4),
        "slot": r.get::<_, i64>(5),
    })
}

const META_COLS: &str = "oracle, subject, options, uri, uri_hash, slot";

/// Oracle metadata for a single oracle PDA, if indexed.
pub async fn get_oracle_meta(client: &Client, oracle: &str) -> Result<Option<serde_json::Value>> {
    let sql = format!("SELECT {META_COLS} FROM oracle_metadata WHERE oracle = $1");
    let rows = client.query(&sql, &[&oracle]).await?;
    Ok(rows.first().map(meta_json))
}

/// Oracle metadata for a batch of oracle PDAs (browse view). Empty input → all
/// indexed metadata (capped), so the list page can prefetch in one call.
pub async fn list_oracle_meta(
    client: &Client,
    oracles: &[String],
    limit: i64,
) -> Result<Vec<serde_json::Value>> {
    let rows = if oracles.is_empty() {
        let sql = format!("SELECT {META_COLS} FROM oracle_metadata ORDER BY slot DESC LIMIT $1");
        client.query(&sql, &[&limit.min(1000)]).await?
    } else {
        let sql = format!("SELECT {META_COLS} FROM oracle_metadata WHERE oracle = ANY($1)");
        client.query(&sql, &[&oracles]).await?
    };
    Ok(rows.iter().map(meta_json).collect())
}

/// The on-chain `uri_hash` (hex) indexed for an oracle — the gate the JSON host
/// checks a POSTed/served JSON against.
pub async fn get_oracle_uri_hash(client: &Client, oracle: &str) -> Result<Option<String>> {
    let rows = client
        .query(
            "SELECT uri_hash FROM oracle_metadata WHERE oracle = $1",
            &[&oracle],
        )
        .await?;
    Ok(rows.first().map(|r| r.get::<_, String>(0)))
}

/// Store the hosted extended-metadata JSON for an oracle (app POST). Upsert:
/// the latest POST wins (the serve path gates it against the on-chain uri_hash).
pub async fn upsert_oracle_meta_json(
    client: &Client,
    oracle: &str,
    json: &str,
    sha256: &str,
) -> Result<()> {
    client
        .execute(
            "INSERT INTO oracle_meta_json (oracle, json, sha256) VALUES ($1,$2,$3)
             ON CONFLICT (oracle) DO UPDATE SET json = EXCLUDED.json, sha256 = EXCLUDED.sha256",
            &[&oracle, &json, &sha256],
        )
        .await?;
    Ok(())
}

/// The hosted JSON + its sha256 for an oracle, if any was POSTed.
pub async fn get_oracle_meta_json(
    client: &Client,
    oracle: &str,
) -> Result<Option<(String, String)>> {
    let rows = client
        .query(
            "SELECT json, sha256 FROM oracle_meta_json WHERE oracle = $1",
            &[&oracle],
        )
        .await?;
    Ok(rows
        .first()
        .map(|r| (r.get::<_, String>(0), r.get::<_, String>(1))))
}

/// Oracles that have a non-empty on-chain `uri` but NO stored JSON matching their
/// `uri_hash` yet — the work list for the autonomous metadata fetcher. Naturally
/// excludes oracles whose JSON was already POSTed (self-hosted) and keeps failed
/// fetches in the set so they retry next tick.
pub async fn oracles_missing_meta_json(
    client: &Client,
    limit: i64,
) -> Result<Vec<(String, String, String)>> {
    let rows = client
        .query(
            "SELECT m.oracle, m.uri, m.uri_hash
             FROM oracle_metadata m
             LEFT JOIN oracle_meta_json j
               ON j.oracle = m.oracle AND j.sha256 = m.uri_hash
             WHERE m.uri <> '' AND j.oracle IS NULL
             ORDER BY m.slot DESC
             LIMIT $1",
            &[&limit],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| {
            (
                r.get::<_, String>(0),
                r.get::<_, String>(1),
                r.get::<_, String>(2),
            )
        })
        .collect())
}
