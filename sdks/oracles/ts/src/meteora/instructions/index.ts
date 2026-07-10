/**
 * Instruction builders for Meteora **DAMM v2** (cp-amm) — the position-based
 * spot-path lifecycle: `initializePool`, `createPosition`, `addLiquidity`,
 * `removeLiquidity`, `swap`, `claimPositionFee`.
 *
 * Each returns a web3.js (classic) `TransactionInstruction` whose
 * `data == [disc, ...borsh_args]` (LE) and whose `keys` are the EXACT account-meta
 * order/roles from the pinned `#[derive(Accounts)]` structs
 * (commit `bdd8a1e355f484b3cff131578a662c560b97b72f`,
 * `programs/cp-amm/src/instructions/…`). Every instruction is Anchor
 * `#[event_cpi]`, so the two trailing accounts (event_authority PDA, program id)
 * are appended by the builders.
 *
 * KEY cp-amm specifics (differ from the MetaDAO AMMs):
 *  - POSITION-based: `initializePool` ALSO mints the first position NFT (it takes
 *    `liquidity` + `sqrt_price` directly); `createPosition` opens an empty one.
 *  - `swap` has NO direction/`swap_type` arg — the trade direction is implicit in
 *    which token account is `inputTokenAccount` vs `outputTokenAccount`. Args are
 *    just `amount_in: u64 ++ minimum_amount_out: u64` (SwapParameters).
 *  - the position NFT mint + its token account live under Token-2022.
 *  - the `Pool` PDA is keyed by a `config` account + the SORTED mint pair.
 */
export * from "./pool.js";
export * from "./liquidity.js";
export * from "./trade.js";
