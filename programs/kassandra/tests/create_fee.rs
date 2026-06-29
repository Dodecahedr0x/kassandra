//! Tests for the dynamic EMA creation fee (Task H2): KASS burned on
//! `create_oracle`, proportional to an EMA of recent creation activity — 0 at
//! genesis, grows with rapid creations, shrinks when idle.

mod common;
use common::*;

use kassandra_program::config::{FEE_EMA_HALFLIFE_SECS, FEE_EMA_INCREMENT, FEE_EMA_SCALE};
use kassandra_program::fee::{bumped_fee_ema, decay_fee_ema, fee_for_ema};

/// Helper: create an oracle with a fresh future deadline and report
/// `(fee_burned, supply_delta)` measured from the creator's KASS balance and
/// the mint supply.
fn create_and_measure(ctx: &mut TestCtx, nonce: u64) -> (u64, u64) {
    let bal_before = ctx.token_balance(ctx.payer_kass);
    let sup_before = ctx.mint_supply(ctx.kass_mint);
    let deadline = ctx.now() + 1_000_000;
    let (_o, res) = ctx.create_oracle(nonce, 2, deadline, 600, [0x33; 32]);
    assert!(res.is_ok(), "create_oracle should succeed: {res:?}");
    let bal_after = ctx.token_balance(ctx.payer_kass);
    let sup_after = ctx.mint_supply(ctx.kass_mint);
    (bal_before - bal_after, sup_before - sup_after)
}

#[test]
fn genesis_create_is_free() {
    let mut ctx = TestCtx::new();
    let (protocol_pda, res) = ctx.init_protocol();
    assert!(res.is_ok(), "init_protocol should succeed: {res:?}");

    let bal_before = ctx.token_balance(ctx.payer_kass);
    let sup_before = ctx.mint_supply(ctx.kass_mint);
    let now = ctx.now();

    let (fee, supply_delta) = create_and_measure(&mut ctx, 0);
    assert_eq!(fee, 0, "genesis creation must be free");
    assert_eq!(supply_delta, 0, "no burn at genesis → supply unchanged");
    assert_eq!(
        ctx.token_balance(ctx.payer_kass),
        bal_before,
        "creator KASS unchanged at genesis"
    );
    assert_eq!(
        ctx.mint_supply(ctx.kass_mint),
        sup_before,
        "mint supply unchanged at genesis"
    );

    let p = ctx.protocol(protocol_pda);
    assert_eq!(
        p.fee_ema, FEE_EMA_INCREMENT,
        "fee_ema bumped to one creation unit"
    );
    assert_eq!(p.last_creation_unix, now, "last_creation_unix == now");
}

#[test]
fn rapid_creates_fee_grows_and_burns() {
    let mut ctx = TestCtx::new();
    let _ = ctx.init_protocol();

    let bal_start = ctx.token_balance(ctx.payer_kass);
    let sup_start = ctx.mint_supply(ctx.kass_mint);

    // Four rapid creations with no clock advance between them: decay is 0, so
    // each adds a full FEE_EMA_INCREMENT and the fee strictly increases.
    let mut fees = Vec::new();
    for nonce in 0..4u64 {
        let (fee, supply_delta) = create_and_measure(&mut ctx, nonce);
        assert_eq!(supply_delta, fee, "each burn reduces supply by the fee");
        fees.push(fee);
    }

    assert_eq!(fees[0], 0, "first (genesis) creation is free");
    for w in fees.windows(2) {
        assert!(
            w[1] > w[0],
            "fee must strictly increase across rapid creations: {fees:?}"
        );
    }

    // The 2nd creation sees fee_ema == 1.0 unit → fee == FEE_PER_EMA_UNIT.
    assert_eq!(fees[1], fee_for_ema(FEE_EMA_INCREMENT));

    // Conservation: creator balance AND mint supply both dropped by Σ fees.
    let total: u64 = fees.iter().sum();
    assert_eq!(ctx.token_balance(ctx.payer_kass), bal_start - total);
    assert_eq!(ctx.mint_supply(ctx.kass_mint), sup_start - total);
}

#[test]
fn idle_gap_shrinks_the_fee() {
    let mut ctx = TestCtx::new();
    let (protocol_pda, _) = ctx.init_protocol();

    // Genesis (free) → fee_ema = 1.0 unit.
    let (f0, _) = create_and_measure(&mut ctx, 0);
    assert_eq!(f0, 0);

    // Immediate (no-gap) creation: fee proportional to the un-decayed EMA.
    let (fee_nogap, _) = create_and_measure(&mut ctx, 1);
    assert!(fee_nogap > 0, "second rapid creation must charge a fee");

    // Idle for two half-lives, then create again: the EMA has decayed, so this
    // fee is strictly LOWER than the no-gap counterpart.
    let ema_before_gap = ctx.protocol(protocol_pda).fee_ema;
    ctx.warp(2 * FEE_EMA_HALFLIFE_SECS);
    let now_at_gap = ctx.now();
    let (fee_gap, _) = create_and_measure(&mut ctx, 2);

    assert!(
        fee_gap < fee_nogap,
        "idle decay must shrink the fee: gap {fee_gap} >= nogap {fee_nogap}"
    );

    // The stored EMA reflects the decay: decayed(ema_before_gap) + one unit.
    let p = ctx.protocol(protocol_pda);
    let expected_decayed = decay_fee_ema(
        ema_before_gap,
        now_at_gap - 2 * FEE_EMA_HALFLIFE_SECS,
        now_at_gap,
    );
    assert_eq!(p.fee_ema, bumped_fee_ema(expected_decayed));
    assert!(
        p.fee_ema < ema_before_gap + FEE_EMA_INCREMENT,
        "decay must make the bumped EMA lower than the no-decay case"
    );
    // Sanity: two half-lives ≈ quarter of the pre-gap EMA.
    assert!((expected_decayed as u128) < (ema_before_gap as u128) / 2);
    let _ = FEE_EMA_SCALE;
}
