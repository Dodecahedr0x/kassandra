# Dead-end Economic Settlement — Design + Plan

> **For Claude:** REQUIRED SUB-SKILL: subagent-driven-development (per-task implement + review).

**Goal:** Fix the dead-end settlement gap so a terminal `InvalidDeadend` oracle (and a governance-resolved-from-dead-end oracle) FULLY DRAINS its `stake_vault` with no stranded funds: **non-slashed principal is returned to each staker; all slashed amounts (`bond_pool`) are BURNED; the `reward_emission` is burned; the creator fee stays burned.** Today the slashed `bond_pool` (and, on the no-facts path, the emission) is never moved out of the vault → stranded.

**The economic rule (decisions locked with the user + documented intent):**
- A dead-end is a non-outcome: **no rewards, no distribution**. Stakers get their **non-slashed principal** back; the governance-chosen `resolved_option` (if `resolve_deadend` ran) is recorded for downstream consumers but does NOT drive reward/slash (documented: design §7/§9, settlement-economics, futarchy F4).
- **Slashed amounts are BURNED on a dead-end** (USER DECISION): misbehavior slashes have no recipient (no winner), so they are burned like the creator fee — including the **no-facts case where every disputing proposer is slashed → all those bonds burned** (deterrent against propose-conflict-then-abandon). `reward_emission` is burned too.
- Net: vault drains to dust; `Σ returned principal + dust + (bond_pool burned) + (emission burned) == Σ stakes + emission`.

## The gap (from investigation, file:line)
- `resolve_deadend.rs:73-79` only sets `resolved_option` + flips `Phase::InvalidDeadend→Resolved`; no token movement. NO marker distinguishes it from an organic Resolved — but it always runs AFTER the oracle is already `InvalidDeadend`, and a dead-end always has `reward_pool == 0`.
- **Case A — no-facts dead-end** (`finalize_facts::finalize_no_facts`, ~`finalize_facts.rs:129-185`): every proposer `disqualified, slashed_amount=bond`, all bonds → `bond_pool`; terminates directly to `InvalidDeadend` and **never burns `reward_emission`**. On claim, every disqualified proposer gets base 0 → **Σ bonds + emission stranded (entire vault)**.
- **Case B — tie/no-survivor dead-end** (`finalize_oracle.rs:256-279`): survivors get `bond − slashed_amount`, agreed-fact stakers get stake; emission already burned here; but **`bond_pool` (disqualified bonds + rejected-fact stakes + approve-voter slashes) is stranded** because `reward_pool == 0` (no reward distribution to carry it out). This strands even on a plain (non-governance) InvalidDeadend when the dispute proceeded then tied.
- Claims (`claims.rs`): `claim_proposer` base = `is_disqualified()?0:bond−slashed_amount` (SAME on both phases — `resolved` only gates the reward); reward terms all scale from `reward_pool` via `reward::reward_buckets`, so **`reward_pool==0 ⇒ every reward term is 0`**.

## Preferred fix (verify first — likely NO marker / NO claims / NO layout / NO SDK change)
KEY INSIGHT to verify: because a governance-resolved dead-end has `reward_pool == 0`, the EXISTING claim path already pays **zero reward + only non-slashed principal** on BOTH `InvalidDeadend` and the governance-resolved `Resolved` state. So the fix is simply to **burn the misrouted funds at the InvalidDeadend finalize sites** so the vault holds only returnable principal:
1. **`finalize_oracle` InvalidDeadend branch (Case B):** in addition to the existing `reward_emission` burn, **burn `bond_pool`** from `stake_vault` (SPL Burn, oracle-PDA-signed) so the slashed amounts leave the vault.
2. **`finalize_facts::finalize_no_facts` (Case A):** **burn `bond_pool` (= Σ bonds) AND `reward_emission`** from `stake_vault` when terminating to `InvalidDeadend` (symmetric with finalize_oracle). This repairs the plain no-facts InvalidDeadend too (currently strands).
3. **Claims unchanged** — verify: on a dead-end, `claim_proposer` returns `bond−slashed_amount` for survivors / 0 for disqualified (their bond was in the now-burned `bond_pool`); fact/vote claims return non-slashed principal. Reward terms are 0 (reward_pool=0). The vault, after the finalize burns, holds exactly the returnable principal → claims drain it to dust.
4. **`resolve_deadend` unchanged** (no token movement — the burns happened at finalize; it just flips the phase + records the option). Update its + `require_terminal`'s docstrings (the "F4 pays stakes-back only, no special-casing" claim is the now-falsified assumption).

**Verification gate (DS1):** confirm by test that with the finalize burns in place, BOTH a plain `InvalidDeadend` AND a governance-resolved-from-dead-end oracle fully drain (survivors/honest stakers get non-slashed principal, disqualified/rejected get 0, vault → dust). If — and only if — a governance-resolved dead-end is found to pay something WRONG via the Resolved path (it shouldn't, since reward_pool=0), fall back to adding a minimal `Oracle.resolved_from_deadend` marker (append at offset 392, re-pin state_layout, update the SDK `decodeOracle`) and branch claims on it. PREFER the no-marker approach; only add the marker if verification proves claims diverge.

## Tasks

### DS1 — Burn the slashed bond_pool + emission at the InvalidDeadend finalize sites (program) + conservation
- Implement the two finalize burns (above): `finalize_oracle` InvalidDeadend branch burns `bond_pool`; `finalize_no_facts` burns `bond_pool` + `reward_emission`. Use the existing oracle-PDA-signed SPL Burn pattern (mirror the emission burn already in `finalize_oracle`). Account lists may need the `kass_mint` + `stake_vault` + token program on `finalize_facts` (the no-facts path) if not already present — add them (ABI change to finalize_facts if needed; update the SDK `finalizeFacts` builder + any harness `*_ix`).
- **Verify the no-marker insight** with tests; if it holds, claims/resolve_deadend/Oracle-layout/SDK-decoder are UNCHANGED.
- Update the **conservation invariant** + docstrings (`claims.rs` require_terminal; resolve_deadend.rs). 
- Tests (`programs/kassandra/tests/`): 
  - **No-facts dead-end:** create→propose conflicting→finalize_no_facts→assert `bond_pool` + emission BURNED (supply down, vault drained of bonds), every (disqualified) proposer claims 0, vault → dust. (User decision: no-facts proposer bonds burned.)
  - **Tie dead-end with slashes:** a dispute that proceeds (facts, a rejected fact / disqualified proposer / slashed voter) then ties → InvalidDeadend → assert survivors/agreed-stakers get non-slashed principal, the `bond_pool` (slashed amounts) is burned, vault → dust.
  - **Governance-resolved dead-end:** the tie/no-facts dead-end then `resolve_deadend(option)` → Resolved → claims still pay non-slashed principal only (no reward), vault → dust; `resolved_option` recorded.
  - **Conservation fuzz arm:** extend `invariants.rs` (the settlement fuzz) to cover the slashed-then-deadend + governance-resolved cases: `Σ returned principal + dust == Σ stakes + emission − (bond_pool burned + emission burned)`. Fuzz disqualified/rejected/slashed combinations.
- `just build` + `cargo test -p kassandra-program` (all green incl. new) + clippy + fmt; if the finalizeFacts ABI changed, `cd sdk && pnpm typecheck && pnpm test` green. Commit `fix(settlement): burn slashed bond_pool + emission on dead-end (no stranding)`.

### DS2 — SDK/E2E touch (only if needed) + docs + covered-vs-deferred
- If DS1 changed the `finalizeFacts` account list or the Oracle layout, update the SDK builder/decoder + parity + add a litesvm/SDK assertion that a governance-resolved dead-end drains. If DS1 needed no SDK change, this is docs-only.
- Update `docs/plans/2026-06-29-kassandra-settlement-economics.md` (or the staker-settlement plan) covered-vs-deferred: dead-end economic settlement now DONE (the burn rule + the no-facts-burn decision); note the governance-resolved path drains. Append the final note to this plan. Commit `docs(settlement): dead-end settlement covered (burn slashed + emission)`.

## Out of scope / deferred
- Dust sweeping / closing the terminal Oracle + stake_vault accounts (the NEXT deferred milestone).
- Any change to the normal (non-dead-end) Resolved economics.

## Execution note
After each task: `just build` + `cargo test -p kassandra-program` green; default `pnpm test` stays green (88) if the SDK is touched. DS1 is the substantive program fix — VERIFY the no-marker insight (reward_pool==0 ⇒ existing claims already correct) before adding any marker; the conservation fuzz over slashed-then-deadend is the proof. Append a DS1/DS2 delta log here.

## DS1 delta log (DONE)

**No-marker insight: HELD, with ONE necessary claims-formula fix (no marker / no layout / no SDK-decoder change).**
- No `Oracle.resolved_from_deadend` marker added; `Oracle::LEN` stays 392; `state_layout`/`decodeOracle` unchanged. A governance-resolved-from-dead-end oracle pays IDENTICALLY to a plain `InvalidDeadend` (both verified to fully drain) because `reward_pool == 0` zeroes every reward term on both phases — confirmed by `deadend_settlement::governance_resolved_deadend_pays_identically_and_drains` and the `slashed_deadend_settlement_conservation` fuzz (both the plain and `governance_resolve` arms).
- **BUT** a genuine conservation issue surfaced that the plan's "claims unchanged" assumption missed: the InvalidDeadend claim path returned the **full stake** to rejected-fact submitters and slashed approve-voters, which is inconsistent with burning their portion of `bond_pool`. The plan's own gate ("rejected-fact submitters get 0 — their funds were in the burned bond_pool") confirms the intended design. Fix: `claims.rs` `claim_fact` / `claim_fact_vote` are now **disposition-based on BOTH terminal phases** (rejected submitter → 0, approve-on-rejected → `stake − slash`, agreed/duplicate → stake), with the reward gated to `Resolved` (0 on a dead-end since `reward_pool == 0`). This is a claim *formula* change only — no marker, no layout/ABI/SDK-decoder change. `claim_proposer` was already correct (`bond − slashed_amount`).

**Exact burns added:**
- `finalize_oracle` (Tie / NoSurvivors → InvalidDeadend): now burns `reward_emission + bond_pool` from `stake_vault` (oracle-PDA-signed SPL Burn). (Emission burn pre-existed; `bond_pool` is new.)
- `finalize_no_facts` (→ InvalidDeadend): now burns `bond_pool (= Σ bonds) + reward_emission` (same signed-Burn pattern).
- No double-count: a challenge `kass_fee` already paid OUT by `settle_challenge` was recorded as `bond − kass_fee` in `bond_pool`, so burning `bond_pool` burns only what is still physically in the vault (verified by `settlement_e2e::e2e_deadend_after_settled_challenge_with_emission`, which now FULLY DRAINS — no stranded 900 dust).

**ABI change (finalize_facts):** added `oracle_nonce: u64` payload + fixed accounts `[1] kass_mint(w) [2] stake_vault(w) [3] token program`, mirroring `finalize_oracle` (the no-facts dead-end needs the oracle-PDA burn signer). Threaded to: the SDK `finalizeFacts` builder (`sdk/src/instructions/dispute.ts`, now takes `nonce` + `kassMint`) + its test + the surfpool e2e callers; the harness `TestCtx::finalize_facts_ix` (new) with the 5 per-test-file `finalize_facts_ix` helpers delegating to it. `Ix::FinalizeFacts` discriminant unchanged.

**Tests:** `deadend_settlement.rs` (no-facts dead-end burns bonds+emission & claims 0; tie-with-slashes burns bond_pool & survivors get `bond − slashed`; governance-resolved drains identically). `invariants.rs` Arm F `slashed_deadend_settlement_conservation` fuzz (fuzzed challenge-disqualify + flip-slash + emission, plain AND governance-resolved, full conservation incl. `kass_fee_out`); Arm A updated for the dead-end burn (vault + bond_pool == total_oracle_stake on a dead-end). `claims.rs` `invalid_deadend_returns_nonslashed_principal` + `flipped_survivor_invalid_deadend_drains` updated to the burn semantics (rejected forfeits; vault drains, no stranding). Harness `seed_terminal_oracle` InvalidDeadend models the post-burn vault (`gross − slashed_pool`, `bond_pool` stamped); `seed_disputed_oracle`/`fund_kass` back vault KASS with mint supply so the real `Burn` has supply to subtract.

**Status:** `just build` + `cargo test -p kassandra-program` (35 bins, incl. new tests + fuzz) green; `cargo clippy` clean; `cargo fmt` applied; `cd sdk && pnpm typecheck` + `pnpm test` (88) green.

## DS2 delta log (DONE)

**Closed the one DS1-review coverage gap: the dead-end FACT/VOTE claim path is now proven by a REAL on-chain finalize-burn-then-claim, not just by logic + the Arm E harness mirror.**

- **New end-to-end test (`settlement_e2e.rs`, real driven path):** `drive_real_fact_vote_deadend` drives the GENUINE front door — `create_oracle → propose×2 (options 0/1) → finalize_proposals → submit_fact×2 → advance_phase → vote_fact×2 → finalize_facts → submit_ai_claim×2 → finalize_ai_claims → finalize_oracle` (only `warp` moves time) — to a Tie dead-end carrying: an **AGREED fact** (approve 2000 clears the 2/3 quorum of `dispute_bond_total == 2000`), a **REJECTED fact** (approve 501 < 1334 quorum, > 0 duplicate) whose lone approve-voter is slashed, and two survivors claiming DISTINCT options (no flip → no proposer slash). A fuzzed-free, deterministic case sized so the rejected approve stake (501) is **ODD** to exercise the floor-vs-ceil margin.
  - `finalize_facts` credits `bond_pool` with the rejected submitter stake (300) + the FLOOR aggregate voter slash `floor(501·1/2) == 250` → `bond_pool == 550` (asserted on-chain). `finalize_oracle` then BURNS `bond_pool + reward_emission` (emission 600 folded in before the terminal finalize).
  - **Claims asserted (real S2):** agreed approve-voter → full stake (2000); agreed submitter → full stake (400); **rejected approve-voter → `stake − ceil(501·1/2) == 501 − 251 == 250`**; **rejected submitter → 0** (still closes + reclaims rent); survivors → `bond − slashed_amount == 1000` each.
  - **Conservation proven end-to-end:** post-burn vault `== Σ stakes − bond_pool`; supply drops by exactly `bond_pool + emission`; the vault drains to **dust == `ceil(501·1/2) − floor(501·1/2) == 1`** (the per-voter ceil-margin — conservation-SAFE, the vault is never short); full equation `Σ returned + dust + bond_pool_burned + emission_burned == Σ stakes + emission` holds.
- **Both terminal phases covered (real driven):** `e2e_fact_vote_deadend_burns_and_drains_real_dispute` (plain `InvalidDeadend`, `resolved_option == CLAIM_OPTION_NONE`) and `e2e_fact_vote_deadend_governance_resolved_pays_identically` (after `resolve_deadend(option) → Resolved`, `reward_pool == 0`, IDENTICAL payouts + identical dust — the no-marker insight now also proven on the fact/vote path).
- **No conservation bug surfaced:** driving the real rejected-fact + slashed-voter Tie dead-end produced bounded dust (== 1, the ceil-margin), never a shortfall. DS1's burn + claims-formula fix is sound end-to-end.
- **No program change** (DS2 is test + docs only). No SDK change beyond DS1's already-threaded `finalize_facts` ABI. Normal-Resolved tests stay green.

**Docs:** updated `docs/plans/2026-06-29-kassandra-settlement-economics.md` (the `InvalidDeadend` row rewritten to the burn-slashed-`bond_pool`-+-emission / return-non-slashed-principal rule + a DONE note with the USER DECISION, governance-drains-identically, the claims-formula fix, and the `finalize_facts` ABI note) and `docs/plans/2026-06-30-kassandra-staker-settlement.md` (a "dead-end settlement follow-up (DONE)" section in its covered-vs-deferred).

## Dead-end settlement: covered vs deferred (final)

**Covered (real instructions, proven end to end):**
- The burn rule at both `InvalidDeadend` finalize sites: `finalize_oracle` (tie / no-survivors) and `finalize_no_facts` burn the slashed `bond_pool` + `reward_emission`; the vault then holds exactly the returnable non-slashed principal.
- The full per-actor dead-end matrix via REAL claims: survivor `bond − slashed_amount`; disqualified → 0; agreed/duplicate fact submitter + voter → stake; rejected submitter → 0; approve-on-rejected → `stake − ceil(slash)`.
- The no-facts USER DECISION (every proposer's bond burned), the tie-with-proposer-slashes path, AND the tie-with-fact/vote-slashes path (rejected fact + slashed approve-voter + agreed fact) — both proposer-slash and fact/vote-slash conservation now driven by real instructions.
- Governance-resolved (`resolve_deadend`) drains IDENTICALLY on both the proposer-slash and fact/vote paths (no marker / no layout / no claims branch — `reward_pool == 0`).
- Conservation incl. the floor-credit-vs-ceil-forfeit margin (bounded, never short) — `deadend_settlement.rs`, `settlement_e2e.rs`, `invariants.rs` Arms E/F.

**Deferred to the NEXT milestone:**
- Dust sweeping / closing the terminal Oracle + `stake_vault` accounts (the conservation-safe sub-unit dust + reclaimable rent remain).
- Any change to the normal (non-dead-end) Resolved economics.
