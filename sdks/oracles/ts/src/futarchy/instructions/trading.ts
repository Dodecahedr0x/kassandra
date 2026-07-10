/**
 * Futarchy v0.6 trading + liquidity builders (`conditionalSwap`, `spotSwap`,
 * `provideLiquidity`) plus MetaDAO's protocol-rake `collectMeteoraDammFees`.
 * All are `#[event_cpi]`. See `../instructions` for conventions.
 */
import { TransactionInstruction } from "@solana/web3.js";

import {
  concatBytes as concat,
  u128LE as u128le,
  u64LE as u64le,
  u8 as u8b,
} from "../../bytes.js";
import { SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID } from "../../constants.js";
import * as mpda from "../../meteora/pda.js";
import type { AddressInput } from "../../pda.js";
import {
  CONDITIONAL_VAULT_ID,
  DAMM_V2_POOL_AUTHORITY,
  DISC,
  FUTARCHY_ID,
  Market,
  METADAO_ADMIN,
  METADAO_MULTISIG_VAULT,
  METEORA_DAMM_V2_ID,
  SQUADS_PERMISSIONLESS_MEMBER,
  SQUADS_V4_ID,
  SwapType,
} from "../constants.js";
import * as fpda from "../pda.js";
import { addr, ata, ro, w } from "./shared.js";

export interface ConditionalSwapArgs {
  dao: AddressInput;
  ammBaseVault: AddressInput;
  ammQuoteVault: AddressInput;
  proposal: AddressInput;
  ammPassBaseVault: AddressInput;
  ammPassQuoteVault: AddressInput;
  ammFailBaseVault: AddressInput;
  ammFailQuoteVault: AddressInput;
  trader: AddressInput;
  userInputAccount: AddressInput;
  userOutputAccount: AddressInput;
  baseVault: AddressInput;
  baseVaultUnderlying: AddressInput;
  quoteVault: AddressInput;
  quoteVaultUnderlying: AddressInput;
  passBaseMint: AddressInput;
  failBaseMint: AddressInput;
  passQuoteMint: AddressInput;
  failQuoteMint: AddressInput;
  question: AddressInput;
  /** `Market.Pass` or `Market.Fail` (Spot is rejected on-chain). */
  market: Market;
  swapType: SwapType;
  inputAmount: bigint | number;
  minOutputAmount: bigint | number;
}

export async function conditionalSwap(a: ConditionalSwapArgs): Promise<TransactionInstruction> {
  const eventAuthority = (await fpda.futarchyEventAuthority()).address;
  const vaultEventAuthority = (await fpda.vaultEventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(FUTARCHY_ID),
    keys: [
      w(a.dao),
      w(a.ammBaseVault),
      w(a.ammQuoteVault),
      ro(a.proposal),
      w(a.ammPassBaseVault),
      w(a.ammPassQuoteVault),
      w(a.ammFailBaseVault),
      w(a.ammFailQuoteVault),
      ro(a.trader, true),
      w(a.userInputAccount),
      w(a.userOutputAccount),
      w(a.baseVault),
      w(a.baseVaultUnderlying),
      w(a.quoteVault),
      w(a.quoteVaultUnderlying),
      w(a.passBaseMint),
      w(a.failBaseMint),
      w(a.passQuoteMint),
      w(a.failQuoteMint),
      ro(CONDITIONAL_VAULT_ID),
      ro(vaultEventAuthority),
      ro(a.question),
      ro(TOKEN_PROGRAM_ID),
      ro(eventAuthority),
      ro(FUTARCHY_ID),
    ],
    data: concat([
      DISC.conditionalSwap,
      u8b(a.market),
      u8b(a.swapType),
      u64le(a.inputAmount),
      u64le(a.minOutputAmount),
    ]),
  });
}

export interface SpotSwapArgs {
  dao: AddressInput;
  userBaseAccount: AddressInput;
  userQuoteAccount: AddressInput;
  ammBaseVault: AddressInput;
  ammQuoteVault: AddressInput;
  user: AddressInput;
  inputAmount: bigint | number;
  swapType: SwapType;
  minOutputAmount: bigint | number;
}

/** `spot_swap` — trades against the embedded spot AMM, cranking its TWAP. */
export async function spotSwap(a: SpotSwapArgs): Promise<TransactionInstruction> {
  const eventAuthority = (await fpda.futarchyEventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(FUTARCHY_ID),
    keys: [
      w(a.dao),
      w(a.userBaseAccount),
      w(a.userQuoteAccount),
      w(a.ammBaseVault),
      w(a.ammQuoteVault),
      ro(a.user, true),
      ro(TOKEN_PROGRAM_ID),
      ro(eventAuthority),
      ro(FUTARCHY_ID),
    ],
    data: concat([
      DISC.spotSwap,
      u64le(a.inputAmount),
      u8b(a.swapType),
      u64le(a.minOutputAmount),
    ]),
  });
}

export interface ProvideLiquidityArgs {
  dao: AddressInput;
  liquidityProvider: AddressInput;
  liquidityProviderBaseAccount: AddressInput;
  liquidityProviderQuoteAccount: AddressInput;
  payer: AddressInput;
  ammBaseVault: AddressInput;
  ammQuoteVault: AddressInput;
  /** Position authority (seeds the AmmPosition PDA; usually == liquidityProvider). */
  positionAuthority: AddressInput;
  quoteAmount: bigint | number;
  maxBaseAmount: bigint | number;
  minLiquidity: bigint | number;
}

/** `provide_liquidity` — seeds the embedded spot AMM (first deposit needs `quote >= MIN_QUOTE_LIQUIDITY`). */
export async function provideLiquidity(a: ProvideLiquidityArgs): Promise<TransactionInstruction> {
  const eventAuthority = (await fpda.futarchyEventAuthority()).address;
  const ammPosition = (await fpda.ammPosition(a.dao, a.positionAuthority)).address;
  return new TransactionInstruction({
    programId: addr(FUTARCHY_ID),
    keys: [
      w(a.dao),
      ro(a.liquidityProvider, true),
      w(a.liquidityProviderBaseAccount),
      w(a.liquidityProviderQuoteAccount),
      w(a.payer, true),
      ro(SYSTEM_PROGRAM_ID),
      w(a.ammBaseVault),
      w(a.ammQuoteVault),
      w(ammPosition),
      ro(TOKEN_PROGRAM_ID),
      ro(eventAuthority),
      ro(FUTARCHY_ID),
    ],
    data: concat([
      DISC.provideLiquidity,
      u64le(a.quoteAmount),
      u64le(a.maxBaseAmount),
      u128le(a.minLiquidity),
      addr(a.positionAuthority).toBytes(),
    ]),
  });
}

export interface CollectMeteoraDammFeesArgs {
  /** The futarchy `Dao` PDA (its Squads multisig + vault are derived from it). */
  dao: AddressInput;
  /**
   * `admin` — writable signer + rent payer for the staged Squads txn/proposal.
   * Under the `production` feature the program requires this == {@link METADAO_ADMIN}
   * (the default).
   */
  admin?: AddressInput;
  /**
   * The NEW Squads transaction index (== `multisig.transaction_index + 1` at
   * call time). Seeds the `squads_multisig_vault_transaction` + `squads_multisig_proposal`.
   */
  transactionIndex: bigint | number;
  // ── Meteora cp-amm (claim_position_fee) accounts ──
  /** The Meteora cp-amm `Pool` the DAO holds a position in. */
  pool: AddressInput;
  /** The DAO's Meteora `Position`. */
  position: AddressInput;
  /** cp-amm token-A vault (base). */
  tokenAVault: AddressInput;
  /** cp-amm token-B vault (quote). */
  tokenBVault: AddressInput;
  /** Base mint (must equal `dao.base_mint`). */
  tokenAMint: AddressInput;
  /** Quote mint (must equal `dao.quote_mint`). */
  tokenBMint: AddressInput;
  /** Token account holding the position NFT. */
  positionNftAccount: AddressInput;
  /** Position owner (per the handler, usually the DAO's Squads vault). */
  owner: AddressInput;
  /**
   * Base fee-recipient token account. Defaults to
   * `ata({@link METADAO_MULTISIG_VAULT}, tokenAMint)` (the on-chain
   * `associated_token::authority` constraint).
   */
  tokenAAccount?: AddressInput;
  /** Quote fee-recipient token account. Defaults to `ata(METADAO_MULTISIG_VAULT, tokenBMint)`. */
  tokenBAccount?: AddressInput;
  /** Token program owning base mint (SPL Token by default). */
  tokenAProgram?: AddressInput;
  /** Token program owning quote mint (SPL Token by default). */
  tokenBProgram?: AddressInput;
  /** The permissionless Squads member (signer). Defaults to {@link SQUADS_PERMISSIONLESS_MEMBER}. */
  permissionlessAccount?: AddressInput;
}

/**
 * `collect_meteora_damm_fees` — **MetaDAO's protocol-rake op; NOT a Kassandra
 * dependency.** The futarchy program sweeps a DAO's Meteora cp-amm position fees
 * to **MetaDAO's OWN protocol vault** (`token_{a,b}_account` are
 * `associated_token::authority = metadao_multisig_vault::ID` = `6awyHMsh…`),
 * gated on **MetaDAO's keeper** (`require_keys_eq!(admin, metadao_admin::ID =
 * tSTp6B6k…)` under `production`). It builds a cp-amm `claim_position_fee` CPI,
 * stages it in the DAO's Squads multisig (`vault_transaction_create` →
 * `proposal_create` → `proposal_approve` → `vault_transaction_execute`, all CPI'd
 * internally), so the DAO's Squads vault signs the actual claim. NO positional
 * args (disc only).
 *
 * **Kassandra does NOT call this.** The DAO collects its OWN Meteora treasury fees
 * ADMIN-FREE — position owned by the DAO's Squads vault, claim authorized by the
 * DAO's own futarchy governance, fees → the DAO's OWN ATAs — via the M1
 * `meteora.claimPositionFee` builder wrapped in a Squads `vault_transaction`
 * (see `test/surfpool/dao-meteora-treasury-e2e.test.ts` / NOTES.md "D1"). This
 * builder is kept only as a faithful, wire-verified pin of the deployed
 * instruction (F2a byte test + F2b live admin-gate + D2 litesvm full-drive).
 *
 * Wire format PINNED from TWO authoritative sources that AGREE exactly (27
 * accounts incl. the `#[event_cpi]` tail, no args):
 *   (a) metaDAOproject/programs@c1000ed84ef6d084203ad2a9c13940fd14feb53c
 *       `programs/futarchy/src/instructions/collect_meteora_damm_fees.rs`
 *       (declare_id == FUTAREL…, Cargo.toml version 0.6.1) + `lib.rs:158`.
 *   (b) the on-chain Anchor IDL of `FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq`
 *       (v0.6.1) — instruction `collectMeteoraDammFees`.
 * disc = sha256("global:collect_meteora_damm_fees")[..8] = 8bd469767e36d68f.
 */
export async function collectMeteoraDammFees(
  a: CollectMeteoraDammFeesArgs,
): Promise<TransactionInstruction> {
  const admin = a.admin ?? METADAO_ADMIN;
  const permissionless = a.permissionlessAccount ?? SQUADS_PERMISSIONLESS_MEMBER;
  const tokenAProgram = a.tokenAProgram ?? TOKEN_PROGRAM_ID;
  const tokenBProgram = a.tokenBProgram ?? TOKEN_PROGRAM_ID;

  const multisig = (await fpda.squadsMultisig(a.dao)).address;
  const squadsVault = (await fpda.squadsVault(multisig, 0)).address;
  const squadsTransaction = (await fpda.squadsTransaction(multisig, a.transactionIndex)).address;
  const squadsProposal = (await fpda.squadsProposal(multisig, a.transactionIndex)).address;
  const eventAuthority = (await fpda.futarchyEventAuthority()).address;

  const dammEventAuthority = (await mpda.eventAuthority()).address;
  // pool_authority::ID is hard-coded in the handler; == the cp-amm [b"pool_authority"] PDA.
  const poolAuthority = DAMM_V2_POOL_AUTHORITY;

  const tokenAAccount = a.tokenAAccount ?? (await ata(METADAO_MULTISIG_VAULT, a.tokenAMint));
  const tokenBAccount = a.tokenBAccount ?? (await ata(METADAO_MULTISIG_VAULT, a.tokenBMint));

  return new TransactionInstruction({
    programId: addr(FUTARCHY_ID),
    keys: [
      w(a.dao),
      w(admin, true),
      w(multisig),
      w(squadsVault),
      w(squadsTransaction),
      w(squadsProposal),
      ro(permissionless, true),
      // meteora_claim_position_fees_accounts (flattened, IDL order)
      ro(METEORA_DAMM_V2_ID),
      ro(dammEventAuthority),
      ro(poolAuthority),
      ro(a.pool),
      w(a.position),
      w(tokenAAccount),
      w(tokenBAccount),
      w(a.tokenAVault),
      w(a.tokenBVault),
      ro(a.tokenAMint),
      ro(a.tokenBMint),
      ro(a.positionNftAccount),
      ro(a.owner),
      ro(tokenAProgram),
      ro(tokenBProgram),
      // trailing programs + event_cpi tail
      ro(SYSTEM_PROGRAM_ID),
      ro(TOKEN_PROGRAM_ID),
      ro(SQUADS_V4_ID),
      ro(eventAuthority),
      ro(FUTARCHY_ID),
    ],
    data: DISC.collectMeteoraDammFees,
  });
}
