/**
 * Squads v4 builders used by the futarchy flow: `vaultTransactionCreate`,
 * `proposalCreate`, `vaultTransactionExecute`. See `../instructions` for
 * conventions.
 */
import { TransactionInstruction } from "@solana/web3.js";
import type { AccountMeta } from "@solana/web3.js";

import { concatBytes as concat, u64LE as u64le, u8 as u8b } from "../../bytes.js";
import { SYSTEM_PROGRAM_ID } from "../../constants.js";
import type { AddressInput } from "../../pda.js";
import { DISC, SQUADS_V4_ID } from "../constants.js";
import * as fpda from "../pda.js";
import { addr, boolb, optString, ro, vecU8, w } from "./shared.js";

export interface VaultTransactionCreateArgs {
  multisig: AddressInput;
  /** Multisig member that initiates (the Dao PDA in the futarchy flow). */
  creator: AddressInput;
  rentPayer: AddressInput;
  /** Index of the new transaction = multisig.transaction_index + 1. */
  transactionIndex: bigint | number;
  vaultIndex?: number;
  ephemeralSigners?: number;
  /** Squads compact `TransactionMessage` bytes (the staged inner CPI). */
  transactionMessage: Uint8Array;
  memo?: string | null;
}

export async function vaultTransactionCreate(a: VaultTransactionCreateArgs): Promise<TransactionInstruction> {
  const transaction = (await fpda.squadsTransaction(a.multisig, a.transactionIndex)).address;
  return new TransactionInstruction({
    programId: addr(SQUADS_V4_ID),
    keys: [
      w(a.multisig),
      w(transaction),
      ro(a.creator, true),
      w(a.rentPayer, true),
      ro(SYSTEM_PROGRAM_ID),
    ],
    data: concat([
      DISC.vaultTransactionCreate,
      u8b(a.vaultIndex ?? 0),
      u8b(a.ephemeralSigners ?? 0),
      vecU8(a.transactionMessage),
      optString(a.memo),
    ]),
  });
}

export interface ProposalCreateArgs {
  multisig: AddressInput;
  creator: AddressInput;
  rentPayer: AddressInput;
  transactionIndex: bigint | number;
  draft?: boolean;
}

export async function proposalCreate(a: ProposalCreateArgs): Promise<TransactionInstruction> {
  const proposal = (await fpda.squadsProposal(a.multisig, a.transactionIndex)).address;
  return new TransactionInstruction({
    programId: addr(SQUADS_V4_ID),
    keys: [
      ro(a.multisig),
      w(proposal),
      ro(a.creator, true),
      w(a.rentPayer, true),
      ro(SYSTEM_PROGRAM_ID),
    ],
    data: concat([DISC.proposalCreate, u64le(a.transactionIndex), boolb(a.draft ?? false)]),
  });
}

export interface VaultTransactionExecuteArgs {
  multisig: AddressInput;
  transactionIndex: bigint | number;
  /** Multisig member that executes (the Dao PDA — has Execute permission). */
  member: AddressInput;
  /**
   * The inner transaction's accounts, in Squads message order (ALT accounts +
   * `message.account_keys`). Composing these is a G3 concern; pass them through.
   */
  remainingAccounts?: AccountMeta[];
}

/** `vault_transaction_execute` — no args; signs the inner CPIs as the vault PDA. */
export async function vaultTransactionExecute(a: VaultTransactionExecuteArgs): Promise<TransactionInstruction> {
  const proposal = (await fpda.squadsProposal(a.multisig, a.transactionIndex)).address;
  const transaction = (await fpda.squadsTransaction(a.multisig, a.transactionIndex)).address;
  return new TransactionInstruction({
    programId: addr(SQUADS_V4_ID),
    keys: [
      ro(a.multisig),
      w(proposal),
      ro(transaction),
      ro(a.member, true),
      ...(a.remainingAccounts ?? []),
    ],
    data: DISC.vaultTransactionExecute,
  });
}
