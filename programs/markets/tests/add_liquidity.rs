//! Integration tests for `add_liquidity` (Ix 11): deposit KASS into an already
//! `Active` market's live cYES/cNO AMM. Drives the real MetaDAO v0.4 binaries in
//! LiteSVM. Covers the balanced pool (no remainder), a skewed pool (remainder
//! returned to the depositor), the accounting fields, and the status/oracle guards.

mod common;
use common::*;
use kassandra_markets_program::state::{Contribution, Market};
use kassandra_markets_sdk::metadao::SwapType;
use solana_sdk::{pubkey::Pubkey, signature::{Keypair, Signer}};

const PROPOSAL: u8 = 1; // kassandra Phase::Proposal (non-terminal)
const MIN_LIQ: u64 = 1_000_000_000; // 1 KASS (9 dp)
const SEED_A: u64 = 600_000_000;
const SEED_B: u64 = 400_000_000;

/// Fund + activate a binary market. Returns (ctx, kass, oracle, market, refs).
fn active_market() -> (TestCtx, Pubkey, Pubkey, Pubkey, MetaDaoRefs) {
    let mut ctx = TestCtx::new();
    ctx.load_metadao();
    let kass = ctx.create_mint(9);
    let authority = Keypair::new();
    let (_cfg, res) = ctx.init_config(authority.pubkey(), kass, MIN_LIQ);
    assert!(res.is_ok(), "init_config: {res:?}");

    let oracle = ctx.seed_kass_oracle(2, PROPOSAL);
    let creator = Keypair::new();
    ctx.svm_airdrop(&creator.pubkey());
    let creator_ata = ctx.create_token_account(kass, creator.pubkey(), 5_000_000_000);
    let (market, res) = ctx.create_market(&creator, oracle, kass, creator_ata, SEED_A);
    assert!(res.is_ok(), "create_market: {res:?}");
    let c2 = Keypair::new();
    ctx.svm_airdrop(&c2.pubkey());
    let c2_ata = ctx.create_token_account(kass, c2.pubkey(), 5_000_000_000);
    let res = ctx.contribute(&c2, market, c2_ata, SEED_B);
    assert!(res.is_ok(), "contribute: {res:?}");

    let refs = ctx.compose_metadao_market(market, oracle, kass);
    let res = ctx.activate(oracle, kass);
    assert!(res.is_ok(), "activate: {res:?}");
    (ctx, kass, oracle, market, refs)
}

#[test]
fn add_liquidity_balanced_pool_no_remainder() {
    let (mut ctx, kass, oracle, market, refs) = active_market();
    let m0: Market = ctx.read_pod(market);
    let (lp_vault, _) = kassandra_markets_sdk::pda::lp_vault(&market);
    let lp0 = ctx.token_balance(lp_vault);

    let depositor = Keypair::new();
    ctx.svm_airdrop(&depositor.pubkey());
    let (dep_cyes, dep_cno, res) =
        ctx.add_liquidity(&depositor, oracle, kass, &refs, 500_000_000);
    assert!(res.is_ok(), "add_liquidity: {res:?}");

    let m1: Market = ctx.read_pod(market);
    let lp1 = ctx.token_balance(lp_vault);
    let lp_new = lp1 - lp0;
    assert!(lp_new > 0, "LP minted into lp_vault");

    // Accounting: lp_total & gross_lp_total grew by lp_new; activation basis frozen.
    assert_eq!(m1.lp_total, m0.lp_total + lp_new, "lp_total += lp_new");
    assert_eq!(m1.gross_lp_total, m0.gross_lp_total + lp_new, "gross_lp_total += lp_new");
    assert_eq!(m1.activation_lp, m0.activation_lp, "activation_lp frozen");
    assert_eq!(
        m1.total_contributed,
        m0.total_contributed + 500_000_000,
        "total_contributed += amount"
    );
    assert_eq!(
        m1.open_contributions,
        m0.open_contributions + 1,
        "new contributor counted"
    );

    // Contribution records late_lp.
    let (contribution, _) = kassandra_markets_sdk::pda::contribution(&market, &depositor.pubkey());
    let c: Contribution = ctx.read_pod(contribution);
    assert_eq!(c.late_lp, lp_new, "contribution.late_lp == lp_new");
    assert_eq!(c.amount, 0, "no funding stake for a pure late LP");

    // Transient holders drained; balanced pool → negligible remainder.
    let (mcyes, _) = kassandra_markets_sdk::pda::market_cyes(&market);
    let (mcno, _) = kassandra_markets_sdk::pda::market_cno(&market);
    assert_eq!(ctx.token_balance(mcyes), 0, "market_cyes drained");
    assert_eq!(ctx.token_balance(mcno), 0, "market_cno drained");
    let escrow = Pubkey::new_from_array(m1.escrow_vault.to_bytes());
    assert_eq!(ctx.token_balance(escrow), 0, "escrow drained");
    // A balanced (untraded) pool deploys both sides nearly in full — only the AMM's
    // round-up dust (a couple base units) is returned.
    assert!(ctx.token_balance(dep_cyes) <= 5, "cYES remainder is dust (balanced)");
    assert!(ctx.token_balance(dep_cno) <= 5, "cNO remainder is dust (balanced)");
}

#[test]
fn add_liquidity_skewed_pool_returns_remainder() {
    let (mut ctx, kass, oracle, market, refs) = active_market();

    // Skew the pool: a trader sells cYES for cNO, so cYES reserve rises above cNO.
    let trader = Keypair::new();
    ctx.svm_airdrop(&trader.pubkey());
    let t_kass = ctx.create_token_account(kass, trader.pubkey(), 5_000_000_000);
    let t_cyes = ctx.create_token_account(refs.yes_mint, trader.pubkey(), 0);
    let t_cno = ctx.create_token_account(refs.no_mint, trader.pubkey(), 0);
    let res = ctx.user_split(&trader, &refs, t_kass, t_cyes, t_cno, 2_000_000_000);
    assert!(res.is_ok(), "trader split: {res:?}");
    let res = ctx.user_swap(&trader, &refs, t_cyes, t_cno, SwapType::Sell, 1_000_000_000, 0);
    assert!(res.is_ok(), "trader swap: {res:?}");
    assert!(
        ctx.token_balance(refs.amm_vault_base) > ctx.token_balance(refs.amm_vault_quote),
        "pool skewed: cYES reserve > cNO reserve"
    );

    let depositor = Keypair::new();
    ctx.svm_airdrop(&depositor.pubkey());
    let (dep_cyes, dep_cno, res) =
        ctx.add_liquidity(&depositor, oracle, kass, &refs, 500_000_000);
    assert!(res.is_ok(), "add_liquidity: {res:?}");

    // Transient holders always end at 0 (remainder returned to the depositor).
    let (mcyes, _) = kassandra_markets_sdk::pda::market_cyes(&market);
    let (mcno, _) = kassandra_markets_sdk::pda::market_cno(&market);
    assert_eq!(ctx.token_balance(mcyes), 0, "market_cyes drained");
    assert_eq!(ctx.token_balance(mcno), 0, "market_cno drained");

    // The heavy side (cNO, since quote deposited fully but base was the binding
    // constraint) is returned to the depositor.
    let remainder = ctx.token_balance(dep_cyes) + ctx.token_balance(dep_cno);
    assert!(remainder > 0, "skewed pool returns a one-sided remainder");

    let (contribution, _) = kassandra_markets_sdk::pda::contribution(&market, &depositor.pubkey());
    let c: Contribution = ctx.read_pod(contribution);
    assert!(c.late_lp > 0, "LP credited despite the remainder");
}

#[test]
fn add_liquidity_rejects_non_active() {
    // A Funding market cannot take AMM liquidity.
    let mut ctx = TestCtx::new();
    ctx.load_metadao();
    let kass = ctx.create_mint(9);
    let authority = Keypair::new();
    let (_cfg, res) = ctx.init_config(authority.pubkey(), kass, MIN_LIQ);
    assert!(res.is_ok(), "init_config: {res:?}");
    let oracle = ctx.seed_kass_oracle(2, PROPOSAL);
    let creator = Keypair::new();
    ctx.svm_airdrop(&creator.pubkey());
    let creator_ata = ctx.create_token_account(kass, creator.pubkey(), 5_000_000_000);
    let (market, res) = ctx.create_market(&creator, oracle, kass, creator_ata, SEED_A);
    assert!(res.is_ok(), "create_market: {res:?}");
    // Compose (so the derived MetaDAO refs exist) but do NOT activate.
    let refs = ctx.compose_metadao_market(market, oracle, kass);

    let depositor = Keypair::new();
    ctx.svm_airdrop(&depositor.pubkey());
    let (_a, _b, res) = ctx.add_liquidity(&depositor, oracle, kass, &refs, 500_000_000);
    assert_eq!(
        custom_code(&res),
        Some(kassandra_markets_program::error::MarketError::NotActive as u32),
        "Funding market rejected"
    );
}

#[test]
fn add_liquidity_rejects_terminal_oracle() {
    let (mut ctx, kass, oracle, _market, refs) = active_market();
    // Oracle resolves → no new liquidity.
    ctx.set_oracle_resolved(oracle, 0);

    let depositor = Keypair::new();
    ctx.svm_airdrop(&depositor.pubkey());
    let (_a, _b, res) = ctx.add_liquidity(&depositor, oracle, kass, &refs, 500_000_000);
    assert_eq!(
        custom_code(&res),
        Some(kassandra_markets_program::error::MarketError::OracleResolved as u32),
        "terminal oracle rejected"
    );
}
