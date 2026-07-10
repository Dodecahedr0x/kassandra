/**
 * Futarchy v0.6 DAO + proposal-lifecycle builders: `initializeDao`,
 * `initializeProposal`, `launchProposal`, `finalizeProposal`. All are
 * `#[event_cpi]`; the trailing (event_authority, program id) accounts are
 * appended here. See `../instructions` for conventions.
 */
import { TransactionInstruction } from "@solana/web3.js";

import {
  concatBytes as concat,
  i16LE as i16le,
  u128LE as u128le,
  u16LE as u16le,
  u32LE as u32le,
  u64LE as u64le,
} from "../../bytes.js";
import { SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID } from "../../constants.js";
import type { AddressInput } from "../../pda.js";
import { CONDITIONAL_VAULT_ID, DISC, FUTARCHY_ID, SQUADS_V4_ID } from "../constants.js";
import * as fpda from "../pda.js";
import { ATA_PROGRAM_ID, addr, ata, ro, w } from "./shared.js";

export interface InitializeDaoArgs {
  /** Signer that seeds the Dao PDA (`[b"dao", dao_creator, nonce]`). */
  daoCreator: AddressInput;
  /** Rent payer + signer. */
  payer: AddressInput;
  /** DAO base token mint (e.g. KASS). */
  baseMint: AddressInput;
  /** DAO quote token mint (must be 6-decimal, e.g. USDC). */
  quoteMint: AddressInput;
  /**
   * Squads `ProgramConfig.treasury`. Read from the on-chain Squads ProgramConfig
   * account (not a PDA). G3 fetches it live; offline callers pass it explicitly.
   */
  squadsProgramConfigTreasury: AddressInput;
  // InitializeDaoParams (Borsh; initial_spending_limit forced to None)
  twapInitialObservation: bigint | number;
  twapMaxObservationChangePerUpdate: bigint | number;
  twapStartDelaySeconds: number;
  minQuoteFutarchicLiquidity: bigint | number;
  minBaseFutarchicLiquidity: bigint | number;
  baseToStake: bigint | number;
  passThresholdBps: number;
  secondsPerProposal: number;
  nonce: bigint | number;
  /**
   * v0.6.1 trailing param `team_sponsored_pass_threshold_bps: i16` (default 0).
   * The DEPLOYED program (0.6.1) added this after `initial_spending_limit`; the
   * v0.6.0 source lacked it. Confirmed against the on-chain Anchor IDL.
   */
  teamSponsoredPassThresholdBps?: number;
  /** v0.6.1 trailing param `team_address: Pubkey` (default the zero/system key). */
  teamAddress?: AddressInput;
}

/**
 * `initialize_dao` (initial_spending_limit == None). Creates the Dao AND, via an
 * internal CPI, the Squads multisig with `create_key == Dao` + vault index 0
 * (see NOTES.md). All Squads/ATA PDAs are derived internally.
 *
 * Arg layout = v0.6.1 DEPLOYED (`InitializeDaoParams` + the two trailing
 * `team_*` fields). 117 bytes for the None spending-limit case.
 */
export async function initializeDao(a: InitializeDaoArgs): Promise<TransactionInstruction> {
  const dao = (await fpda.dao(a.daoCreator, a.nonce)).address;
  const multisig = (await fpda.squadsMultisig(dao)).address;
  const vault = (await fpda.squadsVault(multisig, 0)).address;
  const programConfig = (await fpda.squadsProgramConfig()).address;
  const spendingLimit = (await fpda.squadsSpendingLimit(multisig, dao)).address;
  const baseVault = await ata(dao, a.baseMint);
  const quoteVault = await ata(dao, a.quoteMint);
  const eventAuthority = (await fpda.futarchyEventAuthority()).address;

  const data = concat([
    DISC.initializeDao,
    u128le(a.twapInitialObservation),
    u128le(a.twapMaxObservationChangePerUpdate),
    u32le(a.twapStartDelaySeconds),
    u64le(a.minQuoteFutarchicLiquidity),
    u64le(a.minBaseFutarchicLiquidity),
    u64le(a.baseToStake),
    u16le(a.passThresholdBps),
    u32le(a.secondsPerProposal),
    u64le(a.nonce),
    Uint8Array.from([0]), // Option<InitialSpendingLimit>::None
    i16le(a.teamSponsoredPassThresholdBps ?? 0), // v0.6.1
    addr(a.teamAddress ?? SYSTEM_PROGRAM_ID).toBytes(), // v0.6.1 team_address (default zero key)
  ]);

  return new TransactionInstruction({
    programId: addr(FUTARCHY_ID),
    keys: [
      w(dao),
      ro(a.daoCreator, true),
      w(a.payer, true),
      ro(SYSTEM_PROGRAM_ID),
      ro(a.baseMint),
      ro(a.quoteMint),
      w(multisig),
      ro(vault),
      ro(SQUADS_V4_ID),
      ro(programConfig),
      w(a.squadsProgramConfigTreasury),
      w(spendingLimit),
      w(baseVault),
      w(quoteVault),
      ro(TOKEN_PROGRAM_ID),
      ro(ATA_PROGRAM_ID),
      ro(eventAuthority),
      ro(FUTARCHY_ID),
    ],
    data,
  });
}

export interface InitializeProposalArgs {
  /** The Squads proposal the futarchy proposal references (seeds the Proposal PDA). */
  squadsProposal: AddressInput;
  /** The Squads multisig (v0.6.1 added it as an explicit account). */
  squadsMultisig: AddressInput;
  dao: AddressInput;
  question: AddressInput;
  quoteVault: AddressInput;
  baseVault: AddressInput;
  proposer: AddressInput;
  payer: AddressInput;
}

export async function initializeProposal(a: InitializeProposalArgs): Promise<TransactionInstruction> {
  const proposal = (await fpda.proposal(a.squadsProposal)).address;
  const eventAuthority = (await fpda.futarchyEventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(FUTARCHY_ID),
    keys: [
      w(proposal),
      ro(a.squadsProposal),
      ro(a.squadsMultisig),
      w(a.dao),
      ro(a.question),
      ro(a.quoteVault),
      ro(a.baseVault),
      ro(a.proposer, true),
      w(a.payer, true),
      ro(SYSTEM_PROGRAM_ID),
      ro(eventAuthority),
      ro(FUTARCHY_ID),
    ],
    data: DISC.initializeProposal,
  });
}

export interface LaunchProposalArgs {
  proposal: AddressInput;
  baseVault: AddressInput;
  quoteVault: AddressInput;
  passBaseMint: AddressInput;
  passQuoteMint: AddressInput;
  failBaseMint: AddressInput;
  failQuoteMint: AddressInput;
  dao: AddressInput;
  payer: AddressInput;
  ammPassBaseVault: AddressInput;
  ammPassQuoteVault: AddressInput;
  ammFailBaseVault: AddressInput;
  ammFailQuoteVault: AddressInput;
  /** v0.6.1 added the Squads multisig + proposal as explicit accounts. */
  squadsMultisig: AddressInput;
  squadsProposal: AddressInput;
}

export async function launchProposal(a: LaunchProposalArgs): Promise<TransactionInstruction> {
  const eventAuthority = (await fpda.futarchyEventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(FUTARCHY_ID),
    keys: [
      w(a.proposal),
      ro(a.baseVault),
      ro(a.quoteVault),
      ro(a.passBaseMint),
      ro(a.passQuoteMint),
      ro(a.failBaseMint),
      ro(a.failQuoteMint),
      w(a.dao),
      w(a.payer, true),
      w(a.ammPassBaseVault),
      w(a.ammPassQuoteVault),
      w(a.ammFailBaseVault),
      w(a.ammFailQuoteVault),
      ro(a.squadsMultisig),
      ro(a.squadsProposal),
      ro(SYSTEM_PROGRAM_ID),
      ro(TOKEN_PROGRAM_ID),
      ro(ATA_PROGRAM_ID),
      ro(eventAuthority),
      ro(FUTARCHY_ID),
    ],
    data: DISC.launchProposal,
  });
}

export interface FinalizeProposalArgs {
  proposal: AddressInput;
  dao: AddressInput;
  question: AddressInput;
  squadsProposal: AddressInput;
  squadsMultisig: AddressInput;
  ammPassBaseVault: AddressInput;
  ammPassQuoteVault: AddressInput;
  ammFailBaseVault: AddressInput;
  ammFailQuoteVault: AddressInput;
  ammBaseVault: AddressInput;
  ammQuoteVault: AddressInput;
  quoteVault: AddressInput;
  quoteVaultUnderlying: AddressInput;
  passQuoteMint: AddressInput;
  failQuoteMint: AddressInput;
  passBaseMint: AddressInput;
  failBaseMint: AddressInput;
  baseVault: AddressInput;
  baseVaultUnderlying: AddressInput;
}

export async function finalizeProposal(a: FinalizeProposalArgs): Promise<TransactionInstruction> {
  const eventAuthority = (await fpda.futarchyEventAuthority()).address;
  const vaultEventAuthority = (await fpda.vaultEventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(FUTARCHY_ID),
    keys: [
      w(a.proposal),
      w(a.dao),
      w(a.question),
      w(a.squadsProposal),
      ro(a.squadsMultisig),
      ro(SQUADS_V4_ID),
      w(a.ammPassBaseVault),
      w(a.ammPassQuoteVault),
      w(a.ammFailBaseVault),
      w(a.ammFailQuoteVault),
      w(a.ammBaseVault),
      w(a.ammQuoteVault),
      ro(CONDITIONAL_VAULT_ID),
      ro(vaultEventAuthority),
      ro(TOKEN_PROGRAM_ID),
      w(a.quoteVault),
      w(a.quoteVaultUnderlying),
      w(a.passQuoteMint),
      w(a.failQuoteMint),
      w(a.passBaseMint),
      w(a.failBaseMint),
      w(a.baseVault),
      w(a.baseVaultUnderlying),
      ro(eventAuthority),
      ro(FUTARCHY_ID),
    ],
    data: DISC.finalizeProposal,
  });
}
