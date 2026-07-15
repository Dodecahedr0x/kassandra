//! The market-account reconcile loop: periodically re-snapshot the market
//! program's accounts (getProgramAccounts), upsert them, and prune those closed
//! on-chain.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use carbon_core::account::AccountDecoder;
use solana_pubkey::Pubkey;

use crate::market;
use crate::market::rpc::Rpc as MarketRpc;

/// Periodically re-snapshot the market program's accounts (getProgramAccounts),
/// upsert them into `market_accounts`, and prune those closed on-chain. Runs in
/// BOTH modes: the freshness path in reconcile mode (no ws), and a slower
/// close-pruning pass in subscribe mode (the ws tail never observes a close).
pub async fn market_reconcile_loop(
    rpc: Arc<MarketRpc>,
    client: Arc<tokio_postgres::Client>,
    program_id: Pubkey,
    interval_ms: u64,
) {
    let decoder = market::decoder::KassandraAccountDecoder { program_id };
    loop {
        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        match market_reconcile_once(&rpc, &client, &decoder).await {
            Ok(n) => log::debug!("[market] reconcile: {n} accounts"),
            Err(e) => log::warn!("[market] reconcile failed: {e}"),
        }
    }
}

async fn market_reconcile_once(
    rpc: &MarketRpc,
    client: &tokio_postgres::Client,
    decoder: &market::decoder::KassandraAccountDecoder,
) -> Result<usize> {
    let slot = rpc.get_slot().await? as i64;
    let accounts = rpc.get_program_accounts(&decoder.program_id).await?;
    let mut present: HashSet<String> = HashSet::new();
    let mut n = 0;
    for (pubkey, account) in accounts {
        if let Some(decoded) = decoder.decode_account(&account) {
            let key = pubkey.to_string();
            market::processor::persist(client, &key, &decoded.data, account.data.as_slice(), slot)
                .await;
            present.insert(key);
            n += 1;
        }
    }
    // The ONLY path that removes closed accounts (the subscribe tail can't observe
    // a close). Slot-aware, so a just-created account ahead of this snapshot isn't
    // wrongly dropped.
    market::db::prune(client, slot, &present).await?;
    Ok(n)
}
