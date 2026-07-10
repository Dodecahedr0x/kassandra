/**
 * Post-funding lifecycle instruction builders (Ix 6–10):
 * activate / claimLp / resolveMarket / collectFee / closeMarket.
 *
 * See `../market/index.js` for the module overview. Account orders + payload
 * layouts are mirrored VERBATIM from the verified Rust builders in
 * `sdks/oracles/rust/src/ix.rs` (a mismatch is a silent runtime failure).
 */
import { Address, TransactionInstruction } from "@solana/web3.js";

import {
  EXTERNAL_PROGRAM_IDS,
  Ix,
  MARKET_PROGRAM_ID,
  SYSTEM_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from "../../constants.js";
import * as pda from "../../pda.js";
import type { AddressInput } from "../../pda.js";
import { addr, ro, w, withDisc } from "../payload.js";

// ---------------------------------------------------------------------------
// Activate (Ix 6) — turn a fully-funded Funding market into a live MetaDAO
// cYES/cNO AMM market. Payload = empty. All MetaDAO addresses are caller-supplied
// (the Task-4 flows derive+compose them); the market/escrow PDAs are derived here.
//
// Account order MUST match `sdks/oracles/rust/src/ix.rs::activate` / `processor::activate`:
//  0 market(w)  1 oracle(ro)  2 payer(signer,w)  3 question(ro)  4 vault(w)
//  5 vault_underlying_ata(w)  6 escrow(w,PDA)  7 yes_mint(w)  8 no_mint(w)
//  9 market_cyes(w) 10 market_cno(w) 11 amm(w) 12 lp_mint(w) 13 lp_vault(w)
// 14 amm_vault_base(w) 15 amm_vault_quote(w) 16 cv_event_authority(ro)
// 17 cv_program(ro) 18 amm_event_authority(ro) 19 amm_program(ro)
// 20 token program(ro) 21 system program(ro).
// ---------------------------------------------------------------------------
export interface ActivateArgs {
  /** The market being activated. */
  market: AddressInput;
  /** The market's Kassandra oracle (must be non-terminal). */
  oracle: AddressInput;
  /** Payer (signer): rent for the 3 new market-owned token accounts. */
  payer: AddressInput;
  /** MetaDAO Question (oracle-authority == market). */
  question: AddressInput;
  /** KASS conditional vault. */
  vault: AddressInput;
  /** The vault's KASS ATA (split destination for the underlying). */
  vaultUnderlyingAta: AddressInput;
  /** cYES conditional mint (idx 0). */
  yesMint: AddressInput;
  /** cNO conditional mint (idx 1). */
  noMint: AddressInput;
  /** Market-PDA-owned cYES holder (created here). */
  marketCyes: AddressInput;
  /** Market-PDA-owned cNO holder (created here). */
  marketCno: AddressInput;
  /** The cYES/cNO AMM pool. */
  amm: AddressInput;
  /** The pool's LP mint. */
  lpMint: AddressInput;
  /** Market-PDA-owned LP holder (created here). */
  lpVault: AddressInput;
  /** The AMM's cYES (base) ATA. */
  ammVaultBase: AddressInput;
  /** The AMM's cNO (quote) ATA. */
  ammVaultQuote: AddressInput;
  /** Conditional-vault program event authority. */
  cvEventAuthority: AddressInput;
  /** AMM program event authority. */
  ammEventAuthority: AddressInput;
  /** Conditional-vault program id (defaults to {@link EXTERNAL_PROGRAM_IDS}.conditionalVault). */
  cvProgram?: AddressInput;
  /** AMM program id (defaults to {@link EXTERNAL_PROGRAM_IDS}.ammV04). */
  ammProgram?: AddressInput;
  /** SPL Token program id (defaults to {@link TOKEN_PROGRAM_ID}). */
  tokenProgram?: AddressInput;
  /** System program id (defaults to {@link SYSTEM_PROGRAM_ID}). */
  systemProgram?: AddressInput;
  programId?: Address;
}

export async function activate(args: ActivateArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  const escrow = await pda.escrow(args.market, programId);
  return new TransactionInstruction({
    programId,
    keys: [
      w(addr(args.market)),
      ro(addr(args.oracle)),
      w(addr(args.payer), true),
      ro(addr(args.question)),
      w(addr(args.vault)),
      w(addr(args.vaultUnderlyingAta)),
      w(escrow.address),
      w(addr(args.yesMint)),
      w(addr(args.noMint)),
      w(addr(args.marketCyes)),
      w(addr(args.marketCno)),
      w(addr(args.amm)),
      w(addr(args.lpMint)),
      w(addr(args.lpVault)),
      w(addr(args.ammVaultBase)),
      w(addr(args.ammVaultQuote)),
      ro(addr(args.cvEventAuthority)),
      ro(addr(args.cvProgram ?? EXTERNAL_PROGRAM_IDS.conditionalVault)),
      ro(addr(args.ammEventAuthority)),
      ro(addr(args.ammProgram ?? EXTERNAL_PROGRAM_IDS.ammV04)),
      ro(addr(args.tokenProgram ?? TOKEN_PROGRAM_ID)),
      ro(addr(args.systemProgram ?? SYSTEM_PROGRAM_ID)),
    ],
    data: withDisc(Ix.Activate),
  });
}

// ---------------------------------------------------------------------------
// ClaimLp (Ix 7) — permissionless per-contributor pro-rata claim of the AMM LP
// tokens seeded at activate (the LAST claimer sweeps the entire remaining lp_vault).
// The Contribution is CLOSED (rent → contributor). Payload = empty.
// Accounts: 0 market(w) 1 lp_vault(w,PDA) 2 contribution(w,PDA) 3 contributor_lp_ata(w)
//           4 contributor(w) 5 token program(ro).
// `market` is writable (its open_contributions counter is decremented) and
// `contributor` (== contribution.contributor) receives the closed Contribution's rent.
// ---------------------------------------------------------------------------
export interface ClaimLpArgs {
  /** The Active market (writable — its open_contributions counter decrements). */
  market: AddressInput;
  /**
   * The contributor claiming (seeds the Contribution PDA AND is the rent recipient
   * of the closed Contribution — must equal `contribution.contributor`).
   */
  contributor: AddressInput;
  /** Contributor's LP token account the claim transfers to. */
  contributorLpAta: AddressInput;
  programId?: Address;
}

export async function claimLp(args: ClaimLpArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  const lpVault = await pda.lpVault(args.market, programId);
  const contribution = await pda.contribution(args.market, args.contributor, programId);
  return new TransactionInstruction({
    programId,
    keys: [
      w(addr(args.market)),
      w(lpVault.address),
      w(contribution.address),
      w(addr(args.contributorLpAta)),
      w(addr(args.contributor)),
      ro(TOKEN_PROGRAM_ID),
    ],
    data: withDisc(Ix.ClaimLp),
  });
}

// ---------------------------------------------------------------------------
// ResolveMarket (Ix 8) — permissionless idempotent crank bridging the terminal
// Kassandra oracle result into the market's MetaDAO resolve_question. Payload = empty.
// Accounts: 0 market(w) 1 oracle(ro) 2 question(w) 3 cv_event_authority(ro)
//           4 cv_program(ro).
// ---------------------------------------------------------------------------
export interface ResolveMarketArgs {
  /** The market to resolve (also the CPI signer via seeds). */
  market: AddressInput;
  /** The market's Kassandra oracle (must be Resolved). */
  oracle: AddressInput;
  /** The market's MetaDAO Question. */
  question: AddressInput;
  /** Conditional-vault program event authority. */
  cvEventAuthority: AddressInput;
  /** Conditional-vault program id (defaults to {@link EXTERNAL_PROGRAM_IDS}.conditionalVault). */
  cvProgram?: AddressInput;
  programId?: Address;
}

export async function resolveMarket(args: ResolveMarketArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  return new TransactionInstruction({
    programId,
    keys: [
      w(addr(args.market)),
      ro(addr(args.oracle)),
      w(addr(args.question)),
      ro(addr(args.cvEventAuthority)),
      ro(addr(args.cvProgram ?? EXTERNAL_PROGRAM_IDS.conditionalVault)),
    ],
    data: withDisc(Ix.ResolveMarket),
  });
}

// ---------------------------------------------------------------------------
// CollectFee (Ix 9) — permissionless crank that cuts the protocol fee_bps share
// of a resolved market's accrued LP earnings (program-signed amm::remove_liquidity
// → conditional_vault::redeem_tokens → SPL transfer) into config.fee_destination.
// Payload = empty. The `config` + `escrow` PDAs are derived here; every MetaDAO
// address is caller-supplied (the flow wires them from a decoded Market + Config).
//
// Account order MUST match `sdks/oracles/rust/src/ix.rs::collect_fee` / `processor::collect_fee`:
//  0 market(w)  1 config(ro)  2 fee_destination(w)  3 question(ro)  4 vault(w)
//  5 vault_underlying_ata(w)  6 escrow(w,PDA)  7 yes_mint(w)  8 no_mint(w)
//  9 market_cyes(w) 10 market_cno(w) 11 amm(w) 12 lp_mint(w) 13 lp_vault(w)
// 14 amm_vault_base(w) 15 amm_vault_quote(w) 16 cv_event_authority(ro)
// 17 cv_program(ro) 18 amm_event_authority(ro) 19 amm_program(ro) 20 token program(ro).
// ---------------------------------------------------------------------------
export interface CollectFeeArgs {
  /** The Resolved/Void market being collected (also the CPI signer via seeds). */
  market: AddressInput;
  /** `config.fee_destination`: the KASS token account the fee routes to. */
  feeDestination: AddressInput;
  /** The market's MetaDAO Question (resolved). */
  question: AddressInput;
  /** KASS conditional vault. */
  vault: AddressInput;
  /** The vault's KASS ATA (redeem destination for the underlying). */
  vaultUnderlyingAta: AddressInput;
  /** cYES conditional mint (idx 0). */
  yesMint: AddressInput;
  /** cNO conditional mint (idx 1). */
  noMint: AddressInput;
  /** Market-PDA-owned cYES holder (`pda.cyes(market)`). */
  marketCyes: AddressInput;
  /** Market-PDA-owned cNO holder (`pda.cno(market)`). */
  marketCno: AddressInput;
  /** The cYES/cNO AMM pool. */
  amm: AddressInput;
  /** The pool's LP mint. */
  lpMint: AddressInput;
  /** Market-PDA-owned LP holder. */
  lpVault: AddressInput;
  /** The AMM's cYES (base) ATA. */
  ammVaultBase: AddressInput;
  /** The AMM's cNO (quote) ATA. */
  ammVaultQuote: AddressInput;
  /** Conditional-vault program event authority. */
  cvEventAuthority: AddressInput;
  /** AMM program event authority. */
  ammEventAuthority: AddressInput;
  /** Conditional-vault program id (defaults to {@link EXTERNAL_PROGRAM_IDS}.conditionalVault). */
  cvProgram?: AddressInput;
  /** AMM program id (defaults to {@link EXTERNAL_PROGRAM_IDS}.ammV04). */
  ammProgram?: AddressInput;
  /** SPL Token program id (defaults to {@link TOKEN_PROGRAM_ID}). */
  tokenProgram?: AddressInput;
  programId?: Address;
}

export async function collectFee(args: CollectFeeArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  const config = await pda.config(programId);
  const escrow = await pda.escrow(args.market, programId);
  return new TransactionInstruction({
    programId,
    keys: [
      w(addr(args.market)),
      ro(config.address),
      w(addr(args.feeDestination)),
      ro(addr(args.question)),
      w(addr(args.vault)),
      w(addr(args.vaultUnderlyingAta)),
      w(escrow.address),
      w(addr(args.yesMint)),
      w(addr(args.noMint)),
      w(addr(args.marketCyes)),
      w(addr(args.marketCno)),
      w(addr(args.amm)),
      w(addr(args.lpMint)),
      w(addr(args.lpVault)),
      w(addr(args.ammVaultBase)),
      w(addr(args.ammVaultQuote)),
      ro(addr(args.cvEventAuthority)),
      ro(addr(args.cvProgram ?? EXTERNAL_PROGRAM_IDS.conditionalVault)),
      ro(addr(args.ammEventAuthority)),
      ro(addr(args.ammProgram ?? EXTERNAL_PROGRAM_IDS.ammV04)),
      ro(addr(args.tokenProgram ?? TOKEN_PROGRAM_ID)),
    ],
    data: withDisc(Ix.CollectFee),
  });
}

// ---------------------------------------------------------------------------
// CloseMarket (Ix 10) — permissionless rent reclaim for a fully-settled market.
// SPL-CloseAccounts the Market-PDA-owned token accounts (escrow always;
// cyes/cno/lp_vault iff the market was activated) and closes the Market PDA, all
// rent → the creator. Payload = empty. The pool slots are ALWAYS passed (fixed
// order); the program skips them when the market was never activated.
//
// Account order MUST match `sdks/oracles/rust/src/ix.rs::close_market` / `processor::close_market`:
//  0 market(w) 1 creator(w) 2 escrow(w,PDA) 3 cyes(w,PDA) 4 cno(w,PDA)
//  5 lp_vault(w,PDA) 6 token program(ro).
// ---------------------------------------------------------------------------
export interface CloseMarketArgs {
  /** The terminal market being closed (its Market PDA is reaped, rent → creator). */
  market: AddressInput;
  /** `market.creator` — the recipient of ALL reclaimed rent. */
  creator: AddressInput;
  programId?: Address;
}

export async function closeMarket(args: CloseMarketArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  const escrow = await pda.escrow(args.market, programId);
  const cyes = await pda.cyes(args.market, programId);
  const cno = await pda.cno(args.market, programId);
  const lpVault = await pda.lpVault(args.market, programId);
  return new TransactionInstruction({
    programId,
    keys: [
      w(addr(args.market)),
      w(addr(args.creator)),
      w(escrow.address),
      w(cyes.address),
      w(cno.address),
      w(lpVault.address),
      ro(TOKEN_PROGRAM_ID),
    ],
    data: withDisc(Ix.CloseMarket),
  });
}
