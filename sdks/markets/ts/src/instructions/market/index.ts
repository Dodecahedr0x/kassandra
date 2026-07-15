/**
 * Instruction builders for the kassandra-market instructions.
 *
 * Each builder returns a `@solana/web3.js@3.0.0-rc.2` (classic API)
 * `TransactionInstruction` with:
 *   - `programId` = {@link MARKET_PROGRAM_ID} (overridable per call),
 *   - `keys` = the EXACT account-meta list in the processor's documented order,
 *     each with the correct `isSigner`/`isWritable` role,
 *   - `data` = `[disc, ...payload_LE]`, mirroring the processor's payload bytes.
 *
 * The account orders + payload layouts are mirrored VERBATIM from the verified
 * Rust builders in `sdks/oracles/rust/src/ix.rs` (a mismatch is a silent runtime failure).
 * PDAs the Rust builders derive internally are derived here too (via `../../pda.js`,
 * async), so callers pass only the "real" pubkeys; every builder is `async`.
 *
 * Split by responsibility across `./config.js` (Ix 0–1), `./funding.js` (Ix 2–5),
 * and `./lifecycle.js` (Ix 6–10); this barrel re-exports the identical surface.
 */
export * from "./config.js";
export * from "./funding.js";
export * from "./lifecycle.js";
