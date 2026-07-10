/**
 * D3a — instruction builders for the protocol + oracle-lifecycle instructions.
 *
 * Each builder returns a `@solana/web3.js@3.0.0-rc.2` (classic API)
 * `TransactionInstruction` with:
 *   - `programId` = {@link KASSANDRA_PROGRAM_ID} (overridable per call),
 *   - `keys` = the EXACT account-meta list in the processor's documented order,
 *     each with the correct `isSigner`/`isWritable` role,
 *   - `data` = `[disc, ...payload_LE]`, mirroring the processor's payload bytes.
 *
 * The account orders + payload layouts are mirrored from each processor's
 * `# Accounts` / `# Instruction payload` module-doc header AND cross-checked
 * against the test harness `*_ix` builders in
 * `programs/oracles/tests/common/mod.rs` (the authoritative reference).
 *
 * PDAs are derived internally (via `../pda.js`) so callers pass only the
 * "real" pubkeys; derivation is async, so every builder is `async`.
 */
export * from "./oracle-setup.js";
export * from "./proposals.js";
export * from "./governance.js";
