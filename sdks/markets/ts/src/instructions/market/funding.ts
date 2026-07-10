/**
 * Funding-phase instruction builders (Ix 2–5): create / contribute / cancel / refund.
 *
 * See `../market/index.js` for the module overview. Account orders + payload
 * layouts are mirrored VERBATIM from the verified Rust builders in
 * `sdks/oracles/rust/src/ix.rs` (a mismatch is a silent runtime failure).
 */
import { Address, TransactionInstruction } from "@solana/web3.js";

import { Ix, MARKET_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID } from "../../constants.js";
import * as pda from "../../pda.js";
import type { AddressInput } from "../../pda.js";
import { addr, ro, u64LE, u8, w, withDisc } from "../payload.js";

// ---------------------------------------------------------------------------
// CreateMarket (Ix 2) — create the `outcome_index` binary sub-market for `oracle`,
// its KASS escrow, and the creator's Contribution, transferring `seed_amount`
// KASS in. Binary markets pass `outcomeIndex = 0`; a categorical oracle has one
// sub-market per outcome (the market PDA is keyed by `(oracle, outcomeIndex)`).
// Payload = seed_amount(u64 LE) ++ outcome_index(u8).
// Accounts: 0 config(ro) 1 oracle(ro) 2 market(w,PDA) 3 escrow(w,PDA)
//           4 kass_mint(ro) 5 creator(signer,w) 6 creator_kass_ata(w)
//           7 contribution(w,PDA) 8 token program(ro) 9 system program(ro).
// ---------------------------------------------------------------------------
export interface CreateMarketArgs {
  /** Creator (signer): pays rent + seeds the first contribution. */
  creator: AddressInput;
  /** The Kassandra oracle the market resolves against (seeds the market PDA). */
  oracle: AddressInput;
  /** Canonical KASS mint (== `config.kass_mint`). */
  kassMint: AddressInput;
  /** Creator's KASS token account the seed amount transfers from. */
  creatorKassAta: AddressInput;
  /** KASS seeded into escrow as the creator's contribution. */
  seedAmount: bigint | number;
  /**
   * The oracle outcome this sub-market binds to (`0 <= outcomeIndex <
   * oracle.options_count`); YES = the oracle resolves to this index. Binary
   * markets pass `0`. Keys the market/escrow/contribution PDAs.
   */
  outcomeIndex: number;
  programId?: Address;
}

export async function createMarket(args: CreateMarketArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  const config = await pda.config(programId);
  const market = await pda.market(args.oracle, args.outcomeIndex, programId);
  const escrow = await pda.escrow(market.address, programId);
  const contribution = await pda.contribution(market.address, args.creator, programId);
  return new TransactionInstruction({
    programId,
    keys: [
      ro(config.address),
      ro(addr(args.oracle)),
      w(market.address),
      w(escrow.address),
      ro(addr(args.kassMint)),
      w(addr(args.creator), true),
      w(addr(args.creatorKassAta)),
      w(contribution.address),
      ro(TOKEN_PROGRAM_ID),
      ro(SYSTEM_PROGRAM_ID),
    ],
    data: withDisc(Ix.CreateMarket, u64LE(args.seedAmount), u8(args.outcomeIndex)),
  });
}

// ---------------------------------------------------------------------------
// Contribute (Ix 3) — add `amount` KASS to a Funding market's escrow and
// create-or-increment the contributor's Contribution.
// Payload = amount(u64 LE).
// Accounts: 0 market(w) 1 escrow(w,PDA) 2 contributor(signer,w) 3 contributor_kass_ata(w)
//           4 contribution(w,PDA) 5 token program(ro) 6 system program(ro).
// ---------------------------------------------------------------------------
export interface ContributeArgs {
  /** Contributor (signer): funds the stake + rent for a first-time Contribution. */
  contributor: AddressInput;
  /** The market being contributed to. */
  market: AddressInput;
  /** Contributor's KASS token account the stake transfers from. */
  contributorKassAta: AddressInput;
  /** KASS to stake (raw base units). */
  amount: bigint | number;
  programId?: Address;
}

export async function contribute(args: ContributeArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  const escrow = await pda.escrow(args.market, programId);
  const contribution = await pda.contribution(args.market, args.contributor, programId);
  return new TransactionInstruction({
    programId,
    keys: [
      w(addr(args.market)),
      w(escrow.address),
      w(addr(args.contributor), true),
      w(addr(args.contributorKassAta)),
      w(contribution.address),
      ro(TOKEN_PROGRAM_ID),
      ro(SYSTEM_PROGRAM_ID),
    ],
    data: withDisc(Ix.Contribute, u64LE(args.amount)),
  });
}

// ---------------------------------------------------------------------------
// Cancel (Ix 4) — mark an under-funded Funding market Cancelled once its oracle
// is terminal. Permissionless. Payload = empty.
// Accounts: 0 market(w) 1 oracle(ro).
// ---------------------------------------------------------------------------
export interface CancelArgs {
  /** The market to cancel. */
  market: AddressInput;
  /** The market's Kassandra oracle (must be terminal). */
  oracle: AddressInput;
  programId?: Address;
}

export async function cancel(args: CancelArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  return new TransactionInstruction({
    programId,
    keys: [w(addr(args.market)), ro(addr(args.oracle))],
    data: withDisc(Ix.Cancel),
  });
}

// ---------------------------------------------------------------------------
// Refund (Ix 5) — permissionless per-contributor refund from a Cancelled market.
// The Contribution is CLOSED (rent → contributor). Payload = empty.
// Accounts: 0 market(w) 1 escrow(w,PDA) 2 contribution(w,PDA) 3 contributor_kass_ata(w)
//           4 contributor(w) 5 token program(ro).
// `market` is writable (its open_contributions counter is decremented) and
// `contributor` (== contribution.contributor) receives the closed Contribution's rent.
// ---------------------------------------------------------------------------
export interface RefundArgs {
  /** The Cancelled market (writable — its open_contributions counter decrements). */
  market: AddressInput;
  /**
   * The contributor being refunded (seeds the Contribution PDA AND is the rent
   * recipient of the closed Contribution — must equal `contribution.contributor`).
   */
  contributor: AddressInput;
  /** Contributor's KASS token account the stake refunds to. */
  contributorKassAta: AddressInput;
  programId?: Address;
}

export async function refund(args: RefundArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  const escrow = await pda.escrow(args.market, programId);
  const contribution = await pda.contribution(args.market, args.contributor, programId);
  return new TransactionInstruction({
    programId,
    keys: [
      w(addr(args.market)),
      w(escrow.address),
      w(contribution.address),
      w(addr(args.contributorKassAta)),
      w(addr(args.contributor)),
      ro(TOKEN_PROGRAM_ID),
    ],
    data: withDisc(Ix.Refund),
  });
}
