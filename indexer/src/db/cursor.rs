//! The durable resume cursor (`indexer_cursor`) + the status-endpoint stats.

use anyhow::Result;
use tokio_postgres::Client;

/// The durable resume cursor (signature to pass as the crawler's `until`).
pub async fn get_cursor(client: &Client) -> Result<Option<(String, i64)>> {
    let rows = client
        .query(
            "SELECT signature, slot FROM indexer_cursor WHERE id = 1 AND signature IS NOT NULL",
            &[],
        )
        .await?;
    Ok(rows
        .first()
        .map(|r| (r.get::<_, String>(0), r.get::<_, i64>(1))))
}

/// Promote the durable resume cursor forward.
pub async fn set_cursor(client: &Client, signature: &str, slot: i64) -> Result<()> {
    client
        .execute(
            "INSERT INTO indexer_cursor (id, signature, slot) VALUES (1, $1, $2)
             ON CONFLICT (id) DO UPDATE SET signature = EXCLUDED.signature, slot = EXCLUDED.slot",
            &[&signature, &slot],
        )
        .await?;
    Ok(())
}

/// `(event_count, cursor)` for the status endpoint.
pub async fn stats(client: &Client) -> Result<(i64, Option<(String, i64)>)> {
    let count = client
        .query_one("SELECT COUNT(*)::bigint FROM events", &[])
        .await?
        .get::<_, i64>(0);
    Ok((count, get_cursor(client).await?))
}
