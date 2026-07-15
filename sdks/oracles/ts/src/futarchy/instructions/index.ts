/**
 * Instruction builders for futarchy v0.6 + Squads v4 + conditional_vault.
 *
 * Each builder returns a web3.js (classic) `TransactionInstruction` whose
 * `data == [disc, ...borsh_args]` and whose `keys` are the EXACT account-meta
 * order documented in `../NOTES.md` (sourced from the binary-validated Rust CPI
 * modules + the `metaDAOproject/futarchy@v0.6.0` / `Squads-Protocol/v4` source).
 *
 * All futarchy + conditional_vault instructions are `#[event_cpi]`: the two
 * trailing accounts (event_authority PDA, program id) are appended by the
 * builders. The Meteora DAMM v2 builders live in the sibling `../../meteora`
 * module.
 */
export * from "./conditional-vault.js";
export * from "./dao.js";
export * from "./trading.js";
export * from "./squads.js";
export { ATA_PROGRAM_ID, ata } from "./shared.js";
