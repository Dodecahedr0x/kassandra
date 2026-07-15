//! Integration tests for the db layer against a REAL, ephemeral Postgres —
//! the SQL is Postgres-specific (`JSONB`, `ANY($1)` arrays, `$N` placeholders,
//! `ON CONFLICT … EXCLUDED`), so it must run on the real engine, not a SQLite
//! stand-in that would exercise a rewritten query. Self-skips (never fails)
//! when `TEST_DATABASE_URL` is unset — the dedicated CI `db-it` job provides a
//! Postgres service and sets it; every other run just skips.

use std::sync::Arc;

use tokio_postgres::Client;

use super::{
    connect, get_oracle_meta, get_oracle_meta_json, get_oracle_uri_hash, insert_oracle_meta,
    list_oracle_meta, oracles_missing_meta_json, upsert_oracle_meta_json,
};

async fn test_client() -> Option<Arc<Client>> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    match connect(&url).await {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("[db_it] connect to TEST_DATABASE_URL failed ({e}); skipping");
            None
        }
    }
}

#[tokio::test]
async fn oracle_meta_db_round_trips_against_postgres() {
    let Some(client) = test_client().await else {
        eprintln!("[db_it] TEST_DATABASE_URL unset — skipping Postgres integration test");
        return;
    };
    // Isolate: one comprehensive scenario, clean slate (runs alone; no other db
    // test touches these tables).
    client
        .batch_execute("TRUNCATE oracle_metadata, oracle_meta_json")
        .await
        .expect("truncate");

    let opts = serde_json::json!(["Yes", "No"]);

    // insert + get: subject/options/uri/uriHash/slot round-trip.
    insert_oracle_meta(
        &client,
        "OraA",
        "Q A?",
        &opts,
        "https://h/a.json",
        "aa",
        10,
        "sigA",
    )
    .await
    .unwrap();
    let meta = get_oracle_meta(&client, "OraA")
        .await
        .unwrap()
        .expect("row A");
    assert_eq!(meta["subject"], "Q A?");
    assert_eq!(meta["options"], opts);
    assert_eq!(meta["uri"], "https://h/a.json");
    assert_eq!(meta["uriHash"], "aa");
    assert_eq!(meta["slot"], 10);
    assert!(get_oracle_meta(&client, "absent").await.unwrap().is_none());

    // idempotent: the account is write-once on-chain, so a re-processed ix keeps
    // the first row (ON CONFLICT DO NOTHING).
    insert_oracle_meta(&client, "OraA", "CHANGED", &opts, "u2", "bb", 11, "sigA2")
        .await
        .unwrap();
    assert_eq!(
        get_oracle_meta(&client, "OraA").await.unwrap().unwrap()["subject"],
        "Q A?",
        "second insert must not overwrite"
    );

    // get_oracle_uri_hash.
    assert_eq!(
        get_oracle_uri_hash(&client, "OraA")
            .await
            .unwrap()
            .as_deref(),
        Some("aa")
    );
    assert!(get_oracle_uri_hash(&client, "absent")
        .await
        .unwrap()
        .is_none());

    // list_oracle_meta batch via `ANY($1)` — the Postgres array bind SQLite lacks.
    insert_oracle_meta(&client, "OraB", "Q B?", &opts, "", "bb", 20, "sigB")
        .await
        .unwrap();
    insert_oracle_meta(
        &client,
        "OraC",
        "Q C?",
        &opts,
        "https://h/c.json",
        "cc",
        30,
        "sigC",
    )
    .await
    .unwrap();
    let list = list_oracle_meta(&client, &["OraA".into(), "OraC".into()], 500)
        .await
        .unwrap();
    let mut got: Vec<&str> = list.iter().map(|m| m["oracle"].as_str().unwrap()).collect();
    got.sort();
    assert_eq!(got, vec!["OraA", "OraC"]);

    // meta_json upsert + get; upsert overwrites (latest POST wins).
    upsert_oracle_meta_json(&client, "OraA", "{\"v\":1}", "sha_a")
        .await
        .unwrap();
    assert_eq!(
        get_oracle_meta_json(&client, "OraA")
            .await
            .unwrap()
            .unwrap(),
        ("{\"v\":1}".to_string(), "sha_a".to_string())
    );
    upsert_oracle_meta_json(&client, "OraA", "{\"v\":2}", "sha_a2")
        .await
        .unwrap();
    assert_eq!(
        get_oracle_meta_json(&client, "OraA")
            .await
            .unwrap()
            .unwrap()
            .1,
        "sha_a2"
    );
    assert!(get_oracle_meta_json(&client, "absent")
        .await
        .unwrap()
        .is_none());

    // oracles_missing_meta_json — the LEFT JOIN work list. At this point:
    //   OraA: uri set, uri_hash "aa"; stored json sha "sha_a2" ≠ "aa" → MISSING (stale).
    //   OraB: empty uri                                          → excluded.
    //   OraC: uri set, uri_hash "cc", no stored json            → MISSING.
    let missing: Vec<String> = oracles_missing_meta_json(&client, 100)
        .await
        .unwrap()
        .into_iter()
        .map(|(o, _, _)| o)
        .collect();
    assert!(
        missing.contains(&"OraA".to_string()),
        "stale-hash oracle in work list"
    );
    assert!(
        missing.contains(&"OraC".to_string()),
        "no-json oracle in work list"
    );
    assert!(
        !missing.contains(&"OraB".to_string()),
        "empty-uri oracle excluded"
    );

    // Store MATCHING json for both → the work list drains to empty.
    upsert_oracle_meta_json(&client, "OraA", "{}", "aa")
        .await
        .unwrap();
    upsert_oracle_meta_json(&client, "OraC", "{}", "cc")
        .await
        .unwrap();
    let missing_after = oracles_missing_meta_json(&client, 100).await.unwrap();
    assert!(
        missing_after.is_empty(),
        "all committed json now matches: {missing_after:?}"
    );
}
