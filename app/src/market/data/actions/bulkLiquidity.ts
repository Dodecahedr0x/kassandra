/**
 * Bulk liquidity write ACTIONS (pure ix-builders, NO React) for a categorical
 * oracle's group of sub-markets: deposit into (contribute) or withdraw from
 * (claim-LP) several/all sub-markets at once, as an {@link ActivateStep} sequence
 * driven by `useActionSequence`.
 *
 * The DEFAULT deposit distribution is a UNIFORM share of the entered total across
 * the fundable sub-markets ({@link uniformSplit}). Each step reuses the existing
 * single-market builder, and sets `skipIfLanded: false` — contribute/claim-LP
 * legitimately act on an existing account, so the sequence must never skip them.
 */
import { buildContributeIxs } from "./contribute";
import { buildClaimLpIxs } from "./claimLp";
import type { ActivateStep } from "./activate";
import { toAddress, type AddressInput } from "./ata";
import type { IndexerClient } from "../../lib/indexer";
import { ValidationError } from "../writeAction";

/**
 * Split `total` into `n` as-even-as-possible non-negative base-unit shares. Any
 * indivisible remainder (0..n-1 base units) is spread one-per across the leading
 * shares, so the shares always sum EXACTLY to `total` (no dust lost/created).
 * `total=10, n=3 → [4,3,3]`; `total=10, n=4 → [3,3,2,2]`; `n<=0 → []`.
 */
export function uniformSplit(total: bigint, n: number): bigint[] {
  if (n <= 0) return [];
  if (total < 0n) throw new ValidationError("Amount must be zero or greater.");
  const base = total / BigInt(n);
  let remainder = total - base * BigInt(n);
  const shares: bigint[] = [];
  for (let i = 0; i < n; i++) {
    const extra = remainder > 0n ? 1n : 0n;
    remainder -= extra;
    shares.push(base + extra);
  }
  return shares;
}

/** One sub-market's deposit: its PDA, a display label, and the KASS to contribute. */
export interface BulkContributeEntry {
  market: AddressInput;
  label: string;
  amount: bigint;
}

export interface BuildBulkContributeArgs {
  indexer: IndexerClient;
  /** Canonical KASS mint (shared by every sub-market in the group). */
  kassMint: AddressInput;
  /** Contributor authority (the signer). */
  contributor: AddressInput;
  /** Per-sub-market deposits; entries with `amount <= 0` are dropped. */
  entries: BulkContributeEntry[];
}

/**
 * One contribute step per funded sub-market (dropping zero-amount entries), each
 * flagged `skipIfLanded: false` so a repeat deposit is never skipped.
 */
export async function buildBulkContributeSteps(
  args: BuildBulkContributeArgs,
): Promise<ActivateStep[]> {
  const funded = args.entries.filter((e) => e.amount > 0n);
  if (funded.length === 0) {
    throw new ValidationError("Enter an amount to deposit.");
  }
  return Promise.all(
    funded.map(async (e) => ({
      label: e.label,
      ixs: await buildContributeIxs({
        indexer: args.indexer,
        market: e.market,
        kassMint: args.kassMint,
        contributor: args.contributor,
        amount: e.amount,
      }),
      checkAccount: toAddress("Market", e.market),
      skipIfLanded: false,
    })),
  );
}

/** One sub-market's withdrawal: its PDA, a display label, and its LP mint. */
export interface BulkClaimLpEntry {
  market: AddressInput;
  label: string;
  lpMint: AddressInput;
}

export interface BuildBulkClaimLpArgs {
  indexer: IndexerClient;
  /** The contributor claiming across the group (the signer). */
  contributor: AddressInput;
  entries: BulkClaimLpEntry[];
}

/** One claim-LP step per eligible sub-market, flagged `skipIfLanded: false`. */
export async function buildBulkClaimLpSteps(
  args: BuildBulkClaimLpArgs,
): Promise<ActivateStep[]> {
  if (args.entries.length === 0) {
    throw new ValidationError("Nothing to withdraw.");
  }
  return Promise.all(
    args.entries.map(async (e) => ({
      label: e.label,
      ixs: await buildClaimLpIxs({
        indexer: args.indexer,
        market: e.market,
        contributor: args.contributor,
        lpMint: e.lpMint,
      }),
      checkAccount: toAddress("Market", e.market),
      skipIfLanded: false,
    })),
  );
}
