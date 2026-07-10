/**
 * `conditional_vault` instruction builders: `initializeQuestion`,
 * `initializeConditionalVault`, `splitTokens` / `mergeTokens` / `redeemTokens`,
 * and `resolveQuestion`. See `../instructions` for conventions.
 */
import { TransactionInstruction } from "@solana/web3.js";
import type { AccountMeta } from "@solana/web3.js";

import {
  concatBytes as concat,
  u32LE as u32le,
  u64LE as u64le,
  u8 as u8b,
} from "../../bytes.js";
import { SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID } from "../../constants.js";
import type { AddressInput } from "../../pda.js";
import { CONDITIONAL_VAULT_ID, DISC } from "../constants.js";
import * as fpda from "../pda.js";
import { ATA_PROGRAM_ID, addr, ata, ro, w } from "./shared.js";

export interface InitializeQuestionArgs {
  /** 32-byte question id. */
  questionId: Uint8Array;
  /** Oracle/resolver (for a futarchy proposal this is the Proposal PDA). */
  oracle: AddressInput;
  /** Outcome count (binary futarchy uses 2). */
  numOutcomes: number;
  /** Rent payer + signer. */
  payer: AddressInput;
}

export async function initializeQuestion(a: InitializeQuestionArgs): Promise<TransactionInstruction> {
  const question = (await fpda.question(a.questionId, a.oracle, a.numOutcomes)).address;
  const eventAuthority = (await fpda.vaultEventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(CONDITIONAL_VAULT_ID),
    keys: [
      w(question),
      w(a.payer, true),
      ro(SYSTEM_PROGRAM_ID),
      ro(eventAuthority),
      ro(CONDITIONAL_VAULT_ID),
    ],
    data: concat([
      DISC.initializeQuestion,
      a.questionId,
      addr(a.oracle).toBytes(),
      u8b(a.numOutcomes),
    ]),
  });
}

export interface InitializeConditionalVaultArgs {
  question: AddressInput;
  underlyingMint: AddressInput;
  payer: AddressInput;
  /** Number of outcomes → that many conditional-token mints created (default 2). */
  numOutcomes?: number;
}

export async function initializeConditionalVault(
  a: InitializeConditionalVaultArgs,
): Promise<TransactionInstruction> {
  const n = a.numOutcomes ?? 2;
  const vault = (await fpda.conditionalVault(a.question, a.underlyingMint)).address;
  const vaultUnderlying = await ata(vault, a.underlyingMint);
  const eventAuthority = (await fpda.vaultEventAuthority()).address;
  const condMints: AccountMeta[] = [];
  for (let i = 0; i < n; i++) {
    condMints.push(w((await fpda.conditionalTokenMint(vault, i)).address));
  }
  return new TransactionInstruction({
    programId: addr(CONDITIONAL_VAULT_ID),
    keys: [
      w(vault),
      ro(a.question),
      ro(a.underlyingMint),
      w(vaultUnderlying),
      w(a.payer, true),
      ro(TOKEN_PROGRAM_ID),
      ro(ATA_PROGRAM_ID),
      ro(SYSTEM_PROGRAM_ID),
      ro(eventAuthority),
      ro(CONDITIONAL_VAULT_ID),
      ...condMints,
    ],
    data: DISC.initializeConditionalVault,
  });
}

export interface InteractWithVaultArgs {
  question: AddressInput;
  vault: AddressInput;
  vaultUnderlying: AddressInput;
  /** Signer that owns the user token accounts. */
  authority: AddressInput;
  userUnderlying: AddressInput;
  /** Conditional-token mints, outcome order (index 0..n). */
  conditionalMints: AddressInput[];
  /** User's conditional-token accounts, outcome order (index 0..n). */
  userConditionalAccounts: AddressInput[];
}

async function interactWithVault(disc: Uint8Array, a: InteractWithVaultArgs): Promise<TransactionInstruction> {
  const eventAuthority = (await fpda.vaultEventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(CONDITIONAL_VAULT_ID),
    keys: [
      ro(a.question),
      w(a.vault),
      w(a.vaultUnderlying),
      ro(a.authority, true),
      w(a.userUnderlying),
      ro(TOKEN_PROGRAM_ID),
      ro(eventAuthority),
      ro(CONDITIONAL_VAULT_ID),
      ...a.conditionalMints.map((m) => w(m)),
      ...a.userConditionalAccounts.map((u) => w(u)),
    ],
    data: disc,
  });
}

/** `split_tokens` — mints `amount` of each conditional token, pulls underlying in. */
export function splitTokens(a: InteractWithVaultArgs & { amount: bigint | number }): Promise<TransactionInstruction> {
  return interactWithVault(concat([DISC.splitTokens, u64le(a.amount)]), a);
}

/** `merge_tokens` — burns `amount` of each conditional token, returns underlying. */
export function mergeTokens(a: InteractWithVaultArgs & { amount: bigint | number }): Promise<TransactionInstruction> {
  return interactWithVault(concat([DISC.mergeTokens, u64le(a.amount)]), a);
}

/** `redeem_tokens` — burns the holder's full balances, pays out per resolution. */
export function redeemTokens(a: InteractWithVaultArgs): Promise<TransactionInstruction> {
  return interactWithVault(DISC.redeemTokens, a);
}

export interface ResolveQuestionArgs {
  question: AddressInput;
  /** The question's oracle (signer). */
  oracle: AddressInput;
  /** Binary payout numerators — `[1,0]` pass-side, `[0,1]` fail-side. */
  payoutNumerators: [number, number];
}

export async function resolveQuestion(a: ResolveQuestionArgs): Promise<TransactionInstruction> {
  const eventAuthority = (await fpda.vaultEventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(CONDITIONAL_VAULT_ID),
    keys: [w(a.question), ro(a.oracle, true), ro(eventAuthority), ro(CONDITIONAL_VAULT_ID)],
    data: concat([
      DISC.resolveQuestion,
      u32le(2),
      u32le(a.payoutNumerators[0]),
      u32le(a.payoutNumerators[1]),
    ]),
  });
}
