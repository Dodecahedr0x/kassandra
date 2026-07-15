/**
 * Governance + resolution lifecycle builders: `setGovernance` (Ix=13),
 * `setConfig` (Ix=14) + its {@link SetConfigParams} encoder, `resolveDeadend`
 * (Ix=15), `kassPrice` (Ix=16). See `../lifecycle` for conventions.
 */
import { Address, TransactionInstruction } from "@solana/web3.js";

import { Ix, KASSANDRA_PROGRAM_ID } from "../../constants.js";
import * as pda from "../../pda.js";
import type { AddressInput } from "../../pda.js";
import { concatBytes, i64LE, pubkeyBytes, u64LE, u8, withDisc } from "../payload.js";
import { addr, ro, w } from "./shared.js";

// ---------------------------------------------------------------------------
// SetGovernance (Ix=13) — processor/set_governance.rs
// Accounts: 0 protocol(w) 1 authority(ro,signer) 2 kass_dao(ro).
// Payload (64): dao_authority[32] ++ kass_dao[32].
//
// Task G1: the handoff VALIDATES the linkage against the threaded `kass_dao`
// account — it must equal the payload `kass_dao`, be owned by the futarchy
// program, and carry the `Dao` Anchor discriminator; and the payload
// `dao_authority` must be the Squads v4 vault PDA derived for that DAO.
// ---------------------------------------------------------------------------
export interface SetGovernanceArgs {
  /** Authority (signer): admin pre-handoff, dao_authority post-handoff. */
  authority: AddressInput;
  /** The Squads v4 multisig vault PDA recorded as `dao_authority` (non-zero). */
  daoAuthority: AddressInput;
  /**
   * The futarchy `Dao` account recorded as `kass_dao` (non-zero). Used BOTH as
   * the payload pubkey and the read-only account the processor validates
   * (owner == futarchy program + `Dao` discriminator).
   */
  kassDao: AddressInput;
  programId?: Address;
}

export async function setGovernance(args: SetGovernanceArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  const protocol = await pda.protocol(programId);

  const data = withDisc(
    Ix.SetGovernance,
    pubkeyBytes(args.daoAuthority),
    pubkeyBytes(args.kassDao),
  );

  return new TransactionInstruction({
    programId,
    keys: [
      w(protocol.address),
      ro(addr(args.authority), true),
      ro(addr(args.kassDao)),
    ],
    data,
  });
}

// ---------------------------------------------------------------------------
// SetConfig (Ix=14) — processor/set_config.rs
// Accounts: 0 protocol(w) 1 dao_authority(ro,signer).
// Payload (200): 25 little-endian 8-byte fields in the FIXED order below.
// ---------------------------------------------------------------------------
/**
 * The 25 governable parameters overwritten wholesale by `set_config`, in the
 * EXACT processor order (`set_config.rs` `u64_at`/`i64_at` indices 0..=24).
 * Fields documented as `i64` are encoded signed; the rest unsigned. All are
 * `bigint` so the full u64 range round-trips.
 */
export interface SetConfigParams {
  emissionNum: bigint;
  emissionDen: bigint;
  totalSupplyCap: bigint;
  /** i64 */
  feeEmaHalflife: bigint;
  feePerEmaUnit: bigint;
  feeEmaIncrement: bigint;
  thresholdNum: bigint;
  thresholdDen: bigint;
  marketThresholdNum: bigint;
  marketThresholdDen: bigint;
  flipSlashNum: bigint;
  flipSlashDen: bigint;
  /** i64 */
  phaseWindow: bigint;
  /** i64 */
  proposalWindow: bigint;
  factVoteSlashNum: bigint;
  factVoteSlashDen: bigint;
  rewardProposerWeight: bigint;
  rewardFactWeight: bigint;
  challengeFailUsdcFeeNum: bigint;
  challengeFailUsdcFeeDen: bigint;
  challengeSuccessKassFeeNum: bigint;
  challengeSuccessKassFeeDen: bigint;
  /** Bootstrapping stake-floor curve: fee-EMA below which the floor is 0. */
  stakeFloorEmaThreshold: bigint;
  /** Bootstrapping stake-floor curve: fee-EMA at which the floor reaches max. */
  stakeFloorEmaCap: bigint;
  /** Bootstrapping stake-floor curve: the max floor (KASS base units); 0 = disabled. */
  stakeFloorMax: bigint;
}

/** Encode {@link SetConfigParams} as the 200-byte `set_config` payload (no disc). */
export function encodeSetConfigParams(p: SetConfigParams): Uint8Array {
  return concatBytes([
    u64LE(p.emissionNum), // 0
    u64LE(p.emissionDen), // 1
    u64LE(p.totalSupplyCap), // 2
    i64LE(p.feeEmaHalflife), // 3 (i64)
    u64LE(p.feePerEmaUnit), // 4
    u64LE(p.feeEmaIncrement), // 5
    u64LE(p.thresholdNum), // 6
    u64LE(p.thresholdDen), // 7
    u64LE(p.marketThresholdNum), // 8
    u64LE(p.marketThresholdDen), // 9
    u64LE(p.flipSlashNum), // 10
    u64LE(p.flipSlashDen), // 11
    i64LE(p.phaseWindow), // 12 (i64)
    i64LE(p.proposalWindow), // 13 (i64)
    u64LE(p.factVoteSlashNum), // 14
    u64LE(p.factVoteSlashDen), // 15
    u64LE(p.rewardProposerWeight), // 16
    u64LE(p.rewardFactWeight), // 17
    u64LE(p.challengeFailUsdcFeeNum), // 18
    u64LE(p.challengeFailUsdcFeeDen), // 19
    u64LE(p.challengeSuccessKassFeeNum), // 20
    u64LE(p.challengeSuccessKassFeeDen), // 21
    u64LE(p.stakeFloorEmaThreshold), // 22
    u64LE(p.stakeFloorEmaCap), // 23
    u64LE(p.stakeFloorMax), // 24
  ]);
}

export interface SetConfigArgs {
  /** DAO authority (signer): must equal `protocol.dao_authority`. */
  authority: AddressInput;
  /** The full governable parameter set (overwritten wholesale). */
  params: SetConfigParams;
  programId?: Address;
}

export async function setConfig(args: SetConfigArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  const protocol = await pda.protocol(programId);

  const data = withDisc(Ix.SetConfig, encodeSetConfigParams(args.params));

  return new TransactionInstruction({
    programId,
    keys: [w(protocol.address), ro(addr(args.authority), true)],
    data,
  });
}

// ---------------------------------------------------------------------------
// ResolveDeadend (Ix=15) — processor/resolve_deadend.rs
// Accounts: 0 protocol(ro) 1 oracle(w) 2 dao_authority(ro,signer).
// Payload (1): option u8.
// ---------------------------------------------------------------------------
export interface ResolveDeadendArgs {
  /** The dead-ended oracle to resolve. */
  oracle: AddressInput;
  /** DAO authority (signer): must equal `protocol.dao_authority`. */
  authority: AddressInput;
  /** The winning categorical option (< oracle.options_count). */
  option: number;
  programId?: Address;
}

export async function resolveDeadend(args: ResolveDeadendArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  const protocol = await pda.protocol(programId);

  return new TransactionInstruction({
    programId,
    keys: [
      ro(protocol.address),
      w(addr(args.oracle)),
      ro(addr(args.authority), true),
    ],
    data: withDisc(Ix.ResolveDeadend, u8(args.option)),
  });
}

// ---------------------------------------------------------------------------
// KassPrice (Ix=16) — processor/kass_price.rs
// Accounts: 0 protocol(ro) 1 kass_dao(ro). Payload: empty. Read-only (return data).
// ---------------------------------------------------------------------------
export interface KassPriceArgs {
  /** The futarchy `Dao` account == `protocol.kass_dao`. */
  kassDao: AddressInput;
  programId?: Address;
}

export async function kassPrice(args: KassPriceArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  const protocol = await pda.protocol(programId);

  return new TransactionInstruction({
    programId,
    keys: [ro(protocol.address), ro(addr(args.kassDao))],
    data: withDisc(Ix.KassPrice),
  });
}
