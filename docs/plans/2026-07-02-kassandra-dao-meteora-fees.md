# DAO-Owned Meteora Treasury Fees (fix the MetaDAO-admin dependency) — Design + Plan

> **For Claude:** REQUIRED SUB-SKILL: subagent-driven-development (per-task implement + review).

**Goal:** Fix the design concern surfaced in the Meteora follow-on: the futarchy DAO must collect its OWN Meteora treasury fees WITHOUT any MetaDAO admin. **D1** builds+proves the correct admin-free path (the DAO's Squads vault authorizes a Meteora `claim_position_fee` → the DAO's own ATAs) LIVE on the surfpool fork. **D2** (completeness) full-drives the MetaDAO `collect_meteora_damm_fees` instruction in litesvm via `withSigverify(false)` — proving the F2a wire format end-to-end, while documenting it's MetaDAO's op we don't depend on. **D3** re-documents the F2a builder honestly. NO on-chain program change (SDK/test/docs only).

## The finding (why this milestone exists)
`collect_meteora_damm_fees` (futarchy v0.6.1, `metaDAOproject/programs@c1000ed`) is **MetaDAO's protocol-rake op**, hardwired to BOTH:
- MetaDAO's keeper signer (`require_keys_eq!(admin, metadao_admin::ID=tSTp6B6k…)` under `production`), AND
- MetaDAO's treasury recipient (`associated_token::authority = metadao_multisig_vault::ID = 6awyHMsh…`).
It sweeps a DAO's Meteora LP fees into MetaDAO's OWN vault. **Kassandra must NOT depend on it.** The repo confirms the DAO OWNS its Meteora position (handler comment: *"owner of position - usually the DAO's squads multisig vault"*), so the DAO collects its OWN fees via Meteora `claim_position_fee` (recipient = the OWNER's ATAs, signer = the position owner/delegate = the DAO's Squads vault) — the M1 `claimPositionFee` builder (`sdk/src/meteora/instructions.ts:411`), NO admin. litesvm can bypass sigverify (`withSigverify(false)`, `sdk/node_modules/litesvm/dist/index.d.ts:32`); surfpool cannot impersonate (no cheatcode) — so D2 must be litesvm.

## Decisions (locked with the user)
1. **D1 = the fix, faithful + live on surfpool:** the DAO's Squads vault OWNS the Meteora position and collects its own fees via a governance Squads `vault_transaction` executing `claim_position_fee` → the DAO's ATAs. Not a lighter proof.
2. **D2 = litesvm full-drive of the MetaDAO instruction** (`withSigverify(false)`) — proving the F2a wire format past the admin gate; documented as MetaDAO's op, not a Kassandra dependency.
3. **D3 = honest re-doc** of the F2a builder (MetaDAO protocol-rake, not called by Kassandra; the DAO path is D1).

## Source of truth
- The Squads machinery to REUSE: `sdk/test/surfpool/futarchy-governance-e2e.test.ts` (G3 — drives a real Squads `vault_transaction_create` → proposal → `vault_transaction_execute` on the fork; the DAO member/permissionless member roles at `:54-56`; the inner ix there is `set_config` — D1 swaps it for Meteora `claim_position_fee`). SDK Squads builders: `sdk/src/futarchy/constants.ts` `DISC.vaultTransactionCreate/vaultTransactionExecute/proposalCreate` (`:90-92`) + `sdk/src/futarchy/instructions.ts` (the builders + PDA derivers `squadsMultisig/squadsVault/squadsTransaction/squadsProposal`).
- The Meteora side: `sdk/src/meteora/{instructions,pda,accounts,constants}.ts` — `claimPositionFee` (`:411`; owner/delegate signer, owner's ATAs recipients), `initializePool`/`createPosition`/`addLiquidity`/`swap`, `decodePosition` (fee_a_pending/fee_b_pending). The M2/F1 fork E2E `sdk/test/surfpool/meteora-spot-e2e.test.ts` (fork + clone real Config + drive cp-amm; the token-B fee-side finding).
- The DAO bootstrap: `sdk/test/surfpool/futarchy-meteora-treasury-e2e.test.ts` (F2b — real `initialize_dao`, the Squads multisig/vault derivation, cloning a real Dao) + `harness.ts`.
- The F2a builder to re-doc: `sdk/src/futarchy/instructions.ts` `collectMeteoraDammFees` (`:715`) + `constants.ts` (`:31-61`) + `NOTES.md`.
- litesvm: `sdk/node_modules/litesvm` types — `withSigverify(false)` (`dist/index.d.ts:28-32`), `setAccount`, `addProgram`/`addProgramFromFile`. The SDK already uses litesvm (`sdk/test/*.test.ts`, the `toLiteSvmTransaction` bridge).

## Tasks

### D1 — DAO-owned admin-free Meteora fee claim via the Squads vault (surfpool, live)
- New gated (`KASSANDRA_E2E=1`) `sdk/test/surfpool/dao-meteora-treasury-e2e.test.ts` (or extend futarchy-meteora-treasury). Fork mainnet (real futarchy + Squads v4 + cp-amm). Flow:
  1. Real `initialize_dao` → the DAO + its Squads multisig/vault (mirror F2b/G3 bootstrap).
  2. Stand up a Meteora pool + a POSITION OWNED BY THE DAO's Squads vault (owner = the Squads vault PDA). Determine the cleanest way the DAO's vault comes to own a cp-amm position: (a) create the position with `owner = squadsVault` directly (cp-amm `createPosition`/`initializePool` mints the position NFT to `owner` — if owner can be an arbitrary PDA without signing, set it to the vault), OR (b) create it with a keypair + transfer the position NFT to the vault's token account. Fund it (addLiquidity) + generate fees (swaps, enough for a NONZERO fee — token-B side per the F1 finding). Document which ownership route + verify the position owner == the Squads vault.
  3. Build the Meteora `claim_position_fee` ix (M1 `claimPositionFee`) with owner = the Squads vault PDA + recipients = the DAO's OWN ATAs. WRAP it in a Squads `vault_transaction_create` → `proposal_create` → approve → `vault_transaction_execute` (reuse G3's exact Squads flow/roles — the permissionless member creates+executes; the vault PDA signs the inner ix via execute). Drive all through the REAL programs over RPC (`skipPreflight:false`, confirm-throws).
  4. ASSERT: the DAO's ATAs received the accrued fee (decode before/after, NONZERO), the Position's fee_pending cleared, and NO MetaDAO admin/vault (`tSTp6B6k…`/`6awyHMsh…`) appears anywhere in the flow. This proves the DAO collects its own Meteora fees admin-free, governance-authorized.
- STOP-and-report a genuine blocker (the Squads vault can't be made to own a cp-amm position on the fork, or the vault_transaction can't wrap the Meteora claim) — with the exact error + what was tried. Don't fake it.
- Docs: `sdk/test/surfpool/README.md` + `sdk/src/futarchy/NOTES.md` + `sdk/README.md` — the DAO-owned admin-free Meteora treasury-fee path (D1) is the correct/supported path.
- `cd sdk && pnpm typecheck` + default `pnpm test` (offline green) + gated `KASSANDRA_E2E=1 pnpm exec vitest run test/surfpool/dao-meteora-treasury-e2e.test.ts` (RUN it). Commit `test(e2e): DAO collects its own Meteora fees via Squads vault (admin-free) on forked mainnet`.

### D2 — Full-drive the MetaDAO collect_meteora_damm_fees in litesvm (withSigverify(false))
- New gated `sdk/test/meteora-collect-litesvm.test.ts` (or under a litesvm dir) — a LITESVM test (NOT surfpool; surfpool can't bypass sigverify). 
  1. Load the deployed programs into litesvm: futarchy `FUTAREL…`, cp-amm `cpamd…`, Squads v4. Get their bytes — dump via `solana program dump <id> <file.so>` into a test fixtures dir (document the command + commit the .so's OR fetch them in a setup step), then litesvm `addProgram(id, bytes)`/`addProgramFromFile`. If loading 3 real programs into litesvm is infeasible/too heavy, STOP-and-report with what was tried (this is the D2 risk).
  2. Stand up the state litesvm needs: run `initialize_dao` (real, in litesvm) + a Meteora pool + a DAO-owned position with accrued fees (init/add/swap in litesvm), + the MetaDAO vault (`6awyHMsh…`) recipient ATAs. (Reuse the D1/F2b/M2 setup logic where possible.)
  3. `svm.withSigverify(false)`. Build the F2a `collectMeteoraDammFees` ix with the REAL admin `tSTp6B6k…` marked as a signer (no key needed — sigverify off). Send.
  4. ASSERT it DRIVES TO COMPLETION (past the admin gate): the position's fees are swept to the MetaDAO vault ATAs (decode before/after) — proving the F2a 27-account wire format works end-to-end through the deployed handler (Squads-wrap + Meteora claim CPI), not just to the admin gate. Document CLEARLY: this proves MetaDAO's op works given a bypassed signature; Kassandra does NOT call it (see D1).
- STOP-and-report if litesvm can't host the 3-program multi-CPI environment (a legitimate blocker) — fall back to documenting D2 as attempted + why litesvm couldn't reconstruct it, keeping F2b's reach-the-admin-gate proof as the deployed-verification. Do NOT fake completion.
- Docs: note D2 in NOTES/README (F2a wire format full-drive-verified in litesvm; still not a Kassandra dependency).
- `cd sdk && pnpm typecheck` + default `pnpm test` (offline green — is this litesvm test in the default suite or gated? litesvm is offline, so it CAN be default IF the program .so's are committed fixtures; if it needs network/dump, gate it). Commit `test(litesvm): full-drive MetaDAO collect_meteora_damm_fees via sigverify bypass`.

### D3 — Re-document the F2a builder honestly (fold into D1/D2 commits or a small commit)
- Update `sdk/src/futarchy/NOTES.md` + `sdk/src/futarchy/instructions.ts` (the `collectMeteoraDammFees` docstring) + `sdk/README.md`: `collect_meteora_damm_fees` is **MetaDAO's protocol-rake operation** (fees → MetaDAO's vault `6awyHMsh…`, gated on MetaDAO's keeper `tSTp6B6k…`) — the builder is kept + wire-verified (F2a byte test + F2b reach-the-admin-gate + D2 litesvm full-drive) but is **NOT a Kassandra dependency**. The DAO collects its OWN Meteora treasury fees via the admin-free Squads-vault path (D1). Append the D1/D2/D3 delta + a covered-vs-deferred note to this plan.

## Out of scope / deferred
- On-chain program change (none).
- Removing/changing the F2a builder (keep it — it's correctly pinned + now fully verified; just re-scoped in docs as MetaDAO's op).
- Making Kassandra's governance verdict use Meteora (it correctly uses the embedded AMM).

## Execution note
SDK/test/docs only; default `pnpm test` stays green (offline; D2 litesvm is offline IF program .so fixtures are committed, else gate it). D1 + D2 are INDEPENDENT (D1 = surfpool test; D2 = litesvm test) → can run in parallel. D3 folds into their doc updates. Both D1 + D2 have a real risk (D1: Squads-owns-a-cp-amm-position; D2: 3-program litesvm host) — STOP-report a genuine blocker, never fake. Append a D1/D2/D3 delta log here.

---

## Delta log

### D2 — DONE (litesvm full-drive to COMPLETION; the F2b-deferred sweep is now proven)

litesvm CAN host the 3-program multi-CPI env. `sdk/test/meteora-collect-litesvm.test.ts`
(gated `KASSANDRA_LITESVM_PROGRAMS=1`) loads futarchy (`FUTAREL…`, 1.24 MB),
cp-amm (`cpamd…`, 2.17 MB) and Squads v4 (`SQDS4ep6…`, 1.47 MB) from committed
`.so` fixtures (`sdk/test/fixtures/programs/`, dumped via `solana program dump`)
+ real Squads `ProgramConfig` + a public cp-amm `Config` as account fixtures
(SPL Token / Token-2022 / ATA from litesvm builtins). It builds genuine state with
the real ixs — `initialize_dao` (→ Dao + Squads multisig/vault via the
futarchy→Squads `multisig_create_v2` CPI), cp-amm `initialize_pool` (first position
OWNED BY the DAO's Squads vault, since `creator` is an UncheckedAccount so
`creator == vault`), swaps that accrue a real LP fee — then `svm.withSigverify(false)`
+ the F2a `collectMeteoraDammFees` with the REAL admin `tSTp6B6k…` as a
required-but-UNSIGNED signer (zero-signature slot). The handler drives PAST the
admin gate through the full internal `vault_transaction_create → proposal_create →
proposal_approve → vault_transaction_execute → cp-amm claim_position_fee` chain
(all visible in CPI logs), and the accrued fee is **swept to the MetaDAO vault ATAs**
(`ATA(6awyHMsh…, {base,quote} mint)`): asserted the recipient ATA rose by a nonzero
fee (measured token-B = 951804 raw) and the Position's `fee_b_pending` cleared.

Gotchas resolved: `setComputeUnitLimit(1_400_000)` (the 4-deep CPI chain exceeds
the 200k default), a mainnet-like Clock via `setClock` (cp-amm's fee scheduler /
activation reads it — litesvm's near-zero default overflows the swap math), the
`admin` funded as the Squads rent-payer, and the PUBLIC permissionless member
(`EP3SoC2…`) signed with its published secret (only `admin` is left unsigned).

DEFAULT vs GATED: fixtures committed (offline-reproducible) but the test is GATED
(`.so`s total ~4.9 MB → skip loading them on every default run); default `pnpm test`
stays green with the D2 test SKIPPED (127 passed | 1 skipped). The
`withSigverify(false)` bypass is TEST-ONLY (production still needs the real MetaDAO
keeper); this proves MetaDAO's protocol-rake op, which Kassandra does NOT depend on
(the DAO uses the admin-free D1 path). NOTES.md updated with a distinct D2 subsection.
