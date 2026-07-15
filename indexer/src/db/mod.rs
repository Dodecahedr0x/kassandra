//! Postgres persistence: the event log + the durable resume cursor.

use std::sync::Arc;

use anyhow::Result;
use tokio_postgres::{Client, NoTls};

mod cursor;
mod events;
mod oracle_meta;

#[cfg(test)]
mod tests;

pub use cursor::*;
pub use events::*;
pub use oracle_meta::*;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS events (
  signature    TEXT     NOT NULL,
  ix_index     INT      NOT NULL,
  ix_type      TEXT     NOT NULL,
  discriminant SMALLINT NOT NULL,
  slot         BIGINT   NOT NULL,
  block_time   BIGINT,
  account0     TEXT,
  accounts     JSONB    NOT NULL,
  data_base64  TEXT     NOT NULL,
  PRIMARY KEY (signature, ix_index)
);
CREATE INDEX IF NOT EXISTS events_account0_idx ON events (account0);
CREATE INDEX IF NOT EXISTS events_ix_type_idx  ON events (ix_type);
CREATE INDEX IF NOT EXISTS events_slot_idx      ON events (slot DESC);

-- Durable resume cursor: the crawler is (re)started with `until = signature`, so
-- it re-fetches everything newer than this point. Only promoted forward once the
-- indexer has verifiably caught up to chain head (see the promotion task).
CREATE TABLE IF NOT EXISTS indexer_cursor (
  id        SMALLINT PRIMARY KEY DEFAULT 1,
  signature TEXT,
  slot      BIGINT,
  CONSTRAINT cursor_singleton CHECK (id = 1)
);

-- Oracle metadata INDEXED from the on-chain `oracle_meta` account (via the
-- `write_oracle_meta` instruction): the plaintext subject + option labels are
-- on-chain (authoritative), plus a `uri`/`uri_hash` referencing the extended
-- off-chain JSON. This table is a queryable mirror of chain — clients can also
-- read the account directly.
CREATE TABLE IF NOT EXISTS oracle_metadata (
  oracle    TEXT   PRIMARY KEY,
  subject   TEXT   NOT NULL,
  options   JSONB  NOT NULL,      -- array of option-label strings
  uri       TEXT   NOT NULL,      -- extended-metadata JSON URL (may be empty)
  uri_hash  TEXT   NOT NULL,      -- hex sha256 binding the off-chain JSON
  slot      BIGINT NOT NULL,
  signature TEXT   NOT NULL
);

-- The extended off-chain metadata JSON, hosted for app-created oracles (the app
-- POSTs it at creation; the public app server proxies GET/POST here). Served only
-- when its sha256 matches the on-chain `uri_hash` in `oracle_metadata`.
CREATE TABLE IF NOT EXISTS oracle_meta_json (
  oracle TEXT PRIMARY KEY,
  json   TEXT NOT NULL,
  sha256 TEXT NOT NULL            -- hex sha256 of `json`
);
"#;

/// Connect, spawn the connection driver, and create the schema.
pub async fn connect(database_url: &str) -> Result<Arc<Client>> {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            log::error!("postgres connection error: {e}");
        }
    });
    client.batch_execute(SCHEMA).await?;
    Ok(Arc::new(client))
}
