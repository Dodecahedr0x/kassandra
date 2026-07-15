//! The `events` table: one row per indexed Kassandra instruction.

use anyhow::Result;
use tokio_postgres::Client;

/// One indexed Kassandra instruction.
pub struct Event {
    pub signature: String,
    pub ix_index: i32,
    pub ix_type: String,
    pub discriminant: i16,
    pub slot: i64,
    pub block_time: Option<i64>,
    pub account0: Option<String>,
    /// The instruction's account list, as a JSONB value (jsonb `?` account lookups).
    pub accounts: serde_json::Value,
    pub data_base64: String,
}

/// Insert one event, ignoring duplicates (idempotent re-processing).
pub async fn insert_event(client: &Client, e: &Event) -> Result<()> {
    client
        .execute(
            "INSERT INTO events
               (signature, ix_index, ix_type, discriminant, slot, block_time, account0, accounts, data_base64)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
             ON CONFLICT (signature, ix_index) DO NOTHING",
            &[
                &e.signature,
                &e.ix_index,
                &e.ix_type,
                &e.discriminant,
                &e.slot,
                &e.block_time,
                &e.account0,
                &e.accounts,
                &e.data_base64,
            ],
        )
        .await?;
    Ok(())
}

/// Query events with optional filters, newest first.
pub async fn query_events(
    client: &Client,
    ix_type: Option<&str>,
    account: Option<&str>,
    before_slot: Option<i64>,
    limit: i64,
) -> Result<Vec<serde_json::Value>> {
    let mut where_clauses: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();
    if let Some(t) = ix_type {
        params.push(Box::new(t.to_string()));
        where_clauses.push(format!("ix_type = ${}", params.len()));
    }
    if let Some(a) = account {
        params.push(Box::new(a.to_string()));
        where_clauses.push(format!(
            "(account0 = ${0} OR accounts ? ${0})",
            params.len()
        ));
    }
    if let Some(s) = before_slot {
        params.push(Box::new(s));
        where_clauses.push(format!("slot < ${}", params.len()));
    }
    params.push(Box::new(limit.min(1000)));
    let sql = format!(
        "SELECT signature, ix_index, ix_type, discriminant, slot, block_time, account0, accounts, data_base64
         FROM events {} ORDER BY slot DESC, ix_index DESC LIMIT ${}",
        if where_clauses.is_empty() { String::new() } else { format!("WHERE {}", where_clauses.join(" AND ")) },
        params.len(),
    );
    let refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = params
        .iter()
        .map(|b| b.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();
    let rows = client.query(&sql, &refs).await?;
    Ok(rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "signature": r.get::<_, String>(0),
                "ixIndex": r.get::<_, i32>(1),
                "ixType": r.get::<_, String>(2),
                "discriminant": r.get::<_, i16>(3),
                "slot": r.get::<_, i64>(4),
                "blockTime": r.get::<_, Option<i64>>(5),
                "account0": r.get::<_, Option<String>>(6),
                "accounts": r.get::<_, serde_json::Value>(7),
                "dataBase64": r.get::<_, String>(8),
            })
        })
        .collect())
}
