# Kassandra Settlement Economics — Design Note (build deferred)

**Date:** 2026-06-29
**Status:** Design agreed via brainstorming; BUILD DEFERRED per the dependency-first roadmap
(KASS futarchy → challenge-market rework → staker settlement).

This captures the settlement economic decisions reached so they aren't lost. The
challenge-market portion depends on a KASS price oracle (futarchy) that does not exist yet.

## Roadmap (agreed)
1. **KASS futarchy** — a KASS/USDC price/TWAP oracle (prerequisite for sizing challenge-market liquidity).
2. **Challenge-market rework** — bond-as-AMM-liquidity + directional fees (needs #1).
3. **Staker settlement** — the parts below that are independent of #1/#2.

## Agreed staker-settlement model (independent of the KASS oracle)

- **Trigger: per-staker pull.** After an oracle is terminal (`Resolved`/`InvalidDeadend`), each
  participant calls a `claim_*` for their own account; the program computes entitlement from that
  account + a few oracle-level totals stamped at resolution, transfers KASS from `stake_vault`, and
  **closes the account** (rent to claimant). No global iteration; no one-tx cap.

- **Reward pool (only on `Resolved`):** `reward_pool = bond_pool + reward_emission`, split into
  **cohort buckets** by config weights `PW`/`FW` with `PW > FW`:
  `proposer_bucket = reward_pool·PW/(PW+FW)`, `fact_bucket = reward_pool·FW/(PW+FW)`.
  Each bucket distributed pro-rata within its cohort using two totals stamped at resolution:
  `total_correct_proposer_stake`, `total_approved_fact_stake`. An **empty cohort's bucket rolls into
  the proposer cohort** (always ≥1 winner on `Resolved`).

- **Emissions — minted at CREATION, from the reservoir.** `emission = (TOTAL_SUPPLY_CAP −
  kass_supply) × EMISSION_NUM/EMISSION_DEN`, computed in `create_oracle` AFTER the EMA fee burn (so
  burning boosts the same-tx emission), **minted immediately into the oracle's `stake_vault`** and
  recorded as `Oracle.reward_emission`. This deducts the reservoir at creation (deterministic,
  reservation-serialized — concurrent creations can't over-issue). On `Resolved` it joins
  `bond_pool` in the reward pool; on `InvalidDeadend` it is **burned back** to the reservoir.
  Mint authority = a **program PDA**, baked into the protocol (KASS mint hands authority to it).
  No epochs; the live mint supply is the reservoir state. Burning (fees) refills the reservoir and
  raises future emissions.

- **Per-actor matrix on `Resolved`:**
  | Actor | Outcome |
  |---|---|
  | Correct proposer (survived, `claim_option == resolved_option`) | bond + `bond·proposer_rate` |
  | Wrong-but-survived proposer | bond returned, no reward |
  | Disqualified/slashed proposer (no-show/flip/challenge-fail) | `bond − slashed_amount` |
  | Approved-fact submitter + approve-voter (fact `agreed`) | stake + `stake·fact_rate` |
  | Duplicate-voter / staker on duplicate-dominant fact | stake returned, no reward, no slash |
  | Rejected-fact submitter | 0 (funded `bond_pool`) |
  | Approve-voter on a **rejected** fact | `stake·(1 − FACT_VOTE_SLASH_NUM/DEN)` returned; the
    slashed fraction was added to `bond_pool` by `finalize_facts` at finalize time (no per-vote iteration:
    it uses the rejected fact's aggregate `approve_stake`) |

- **`InvalidDeadend`:** a non-outcome — **no rewards, no distribution**. Stakers reclaim their
  **non-slashed principal**; all **slashed amounts (`bond_pool`) are BURNED** along with the
  `reward_emission`; the creator fee stays burned. (BUILT — see the dead-end-settlement note below;
  this supersedes the earlier "every staker reclaims full stake" sketch, which mis-modelled the
  slashed-then-deadend case.) Concretely, per actor on a dead-end:
  - Survivor proposer → `bond − slashed_amount` (honest → full bond; flip-slashed → only the
    un-slashed remainder); disqualified proposer → 0 (its bond was in the burned `bond_pool`, and a
    challenge-disqualify already paid the `kass_fee` out).
  - Agreed / duplicate-dominant fact submitter + voter → full stake (no reward).
  - **Rejected-fact submitter → 0**; **approve-voter on a rejected fact → `stake − ceil(stake·
    FACT_VOTE_SLASH_NUM/DEN)`** — the slashed fraction funded the now-burned `bond_pool`. The
    aggregate credited to `bond_pool` at finalize is the FLOOR `floor(Σ approve·num/den)`; the
    per-voter forfeit is the CEIL, so the vault retains bounded conservation-safe dust (never short).

> **Dead-end economic settlement — DONE (2026-07-01).** The dead-end gap is closed: the slashed
> `bond_pool` (disqualified bonds + rejected-fact submitter stakes + rejected-fact approve-voter
> slashes) + the `reward_emission` are BURNED at the `InvalidDeadend` finalize sites (`finalize_oracle`
> tie/no-survivors branch; `finalize_facts` no-facts branch), leaving the vault holding exactly the
> returnable non-slashed principal for the S2 pull-claims to drain to dust. **USER DECISION:** on the
> no-facts dead-end, every disputing proposer's bond is BURNED (no recipient — a deterrent against
> propose-conflict-then-abandon), not redistributed. **Governance-resolved path drains IDENTICALLY:**
> a `resolve_deadend(option)` flips `InvalidDeadend → Resolved` and records `resolved_option`, but
> `reward_pool == 0` zeroes every reward term, so claims pay non-slashed principal only on BOTH
> terminal phases — **no marker, no layout change, no claims branch** needed. **Claims-formula fix:**
> `claim_fact` / `claim_fact_vote` apply the fact disposition (rejected submitter → 0, approve-on-
> rejected → `stake − ceil(slash)`, agreed/duplicate → stake) on BOTH terminal phases, matching
> Resolved (the reward term is gated to Resolved and is 0 on a dead-end). **ABI note:** `finalize_facts`
> gained an `oracle_nonce` payload + fixed `kass_mint`/`stake_vault`/token-program accounts (the burn
> signer), mirroring `finalize_oracle`; threaded to the SDK `finalizeFacts` builder. Covered by
> `deadend_settlement.rs`, `settlement_e2e.rs` (incl. the real-driven fact/vote dead-end tests
> `e2e_fact_vote_deadend_*`), and the `invariants.rs` conservation fuzz (Arms E/F). DEFERRED to the
> NEXT milestone: dust sweeping / closing the terminal Oracle + `stake_vault` accounts.

- **Challengers:** paid by the challenge market (see deferred section) — a **KASS fee from the bond
  on success**; NOT from `bond_pool` directly.

- **Instructions (staker settlement):** `claim_proposer`, `claim_fact` (submitter),
  `claim_fact_vote`, `close_ai_claim` (permissionless rent reclaim), and the resolution-time changes
  to stamp totals + mint/burn emission. `finalize_facts` extended to add the rejected-fact
  approve-vote slash to `bond_pool` and accumulate `total_approved_fact_stake`. `finalize_oracle`
  stamps `total_correct_proposer_stake` + finalizes `reward_pool`.

- **Conservation invariant (updated):** physical KASS in `stake_vault` (+ conditional-vault-held KASS
  for live challenges) must equal Σ unclaimed entitlements + `bond_pool` remainder; emission mint at
  creation and burn-back on dead-end are the only supply changes besides the fee burn. Fuzz this.

## Deferred — challenge-market settlement (needs the KASS futarchy)

The pass/fail decision-market AMMs need two-sided liquidity:
- **KASS side comes from the proposer's bond** (split into pass-KASS/fail-KASS as liquidity).
- **Challenger provides the matching USDC** (split into pass-USDC/fail-USDC).
- Sizing the matching USDC needs a **KASS/USDC price** — provided by a **KASS futarchy AMM TWAP**
  (the dependency being built first). Mis-sized initial liquidity = free arbitrage.

On resolution: **KASS liquidity → back to the bond**, **USDC → back to the challenger**, minus a
directional fee:
- **Challenge failed** (claim survived): a **USDC fee** from the challenger → the **proposer**.
- **Challenge succeeded** (claim disqualified): a **KASS fee** from the bond → the **challenger**;
  the bond remainder → `bond_pool`.

This **reworks** the merged `open_challenge`/`settle_challenge` (which currently hold the bond's split
KASS idle in oracle accounts and provision AMMs off-chain in tests, with no directional fees), and
is gated on the KASS futarchy. `redeem_tokens` recovers the program-held conditional KASS to
`stake_vault`; no double-count (it realizes the existing `bond_pool` counter set by `settle_challenge`).
