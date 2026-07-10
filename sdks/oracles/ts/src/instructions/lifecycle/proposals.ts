/**
 * Proposal-flow lifecycle builders: `propose` (Ix=11), `finalizeProposals`
 * (Ix=12), `advancePhase` (Ix=7). See `../lifecycle` for conventions.
 */
import { Address, TransactionInstruction } from "@solana/web3.js";

import { Ix, KASSANDRA_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID } from "../../constants.js";
import * as pda from "../../pda.js";
import type { AddressInput } from "../../pda.js";
import { u64LE, u8, withDisc } from "../payload.js";
import { addr, ro, w } from "./shared.js";

// ---------------------------------------------------------------------------
// Propose (Ix=11) — processor/propose.rs
// Accounts: 0 oracle(w) 1 proposer(w,PDA) 2 authority(w,signer) 3 authority_kass(w)
//           4 stake_vault(w,PDA) 5 token program(ro) 6 system program(ro).
// Payload (9): option u8 ++ bond u64.
// ---------------------------------------------------------------------------
export interface ProposeArgs {
  /** The oracle being proposed against. */
  oracle: AddressInput;
  /** Proposer authority (signer): funds rent + bond-transfer authority. */
  authority: AddressInput;
  /** Authority's KASS token account — the bond source. */
  authorityKass: AddressInput;
  /** Categorical option proposed (< oracle.options_count). */
  option: number;
  /** KASS bond escrowed into the stake vault (> 0). */
  bond: bigint | number;
  programId?: Address;
}

export async function propose(args: ProposeArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  const oracle = addr(args.oracle);
  const proposer = await pda.proposer(oracle, addr(args.authority), programId);
  const stakeVault = await pda.stakeVault(oracle, programId);

  const data = withDisc(Ix.Propose, u8(args.option), u64LE(args.bond));

  return new TransactionInstruction({
    programId,
    keys: [
      w(oracle),
      w(proposer.address),
      w(addr(args.authority), true),
      w(addr(args.authorityKass)),
      w(stakeVault.address),
      ro(TOKEN_PROGRAM_ID),
      ro(SYSTEM_PROGRAM_ID),
    ],
    data,
  });
}

// ---------------------------------------------------------------------------
// FinalizeProposals (Ix=12) — processor/finalize_proposals.rs
// Accounts: 0 oracle(w), then the FULL proposer set as a READ-ONLY tail.
// Payload: empty.
// ---------------------------------------------------------------------------
export interface FinalizeProposalsArgs {
  /** The oracle to finalize. */
  oracle: AddressInput;
  /** The FULL proposer-PDA set (exactly `proposer_count`), each read-only. */
  proposers: ReadonlyArray<AddressInput>;
  programId?: Address;
}

export async function finalizeProposals(
  args: FinalizeProposalsArgs,
): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  return new TransactionInstruction({
    programId,
    keys: [w(addr(args.oracle)), ...args.proposers.map((p) => ro(addr(p)))],
    data: withDisc(Ix.FinalizeProposals),
  });
}

// ---------------------------------------------------------------------------
// AdvancePhase (Ix=7) — processor/advance_phase.rs
// Accounts: 0 oracle(w). Payload: empty. (Permissionless: no signer.)
// ---------------------------------------------------------------------------
export interface AdvancePhaseArgs {
  /** The oracle to tick FactProposal -> FactVoting. */
  oracle: AddressInput;
  programId?: Address;
}

export async function advancePhase(args: AdvancePhaseArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  return new TransactionInstruction({
    programId,
    keys: [w(addr(args.oracle))],
    data: withDisc(Ix.AdvancePhase),
  });
}
