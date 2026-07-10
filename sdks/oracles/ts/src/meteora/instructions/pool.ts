/**
 * Meteora DAMM v2 (cp-amm) pool + position creation builders: `initializePool`
 * (also mints the first position NFT) and `createPosition` (opens an empty one).
 */
import { TransactionInstruction } from "@solana/web3.js";

import { concatBytes as concat, u128LE as u128le } from "../../bytes.js";
import { SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID } from "../../constants.js";
import type { AddressInput } from "../../pda.js";
import { DISC, METEORA_DAMM_V2_ID, TOKEN_2022_PROGRAM_ID } from "../constants.js";
import * as mpda from "../pda.js";
import { addr, optionU64le, ro, w } from "./shared.js";

export interface InitializePoolArgs {
  /** Pool creator (recorded as `Pool.creator`; UncheckedAccount, not a signer). */
  creator: AddressInput;
  /** Fee/rent payer + signer. */
  payer: AddressInput;
  /** New position-NFT mint — a fresh keypair the caller must also sign with. */
  positionNftMint: AddressInput;
  /** The `Config` account the pool belongs to (fee/price-range params). */
  config: AddressInput;
  /** Token A mint. */
  tokenAMint: AddressInput;
  /** Token B mint. */
  tokenBMint: AddressInput;
  /** Payer's token-A source account. */
  payerTokenA: AddressInput;
  /** Payer's token-B source account. */
  payerTokenB: AddressInput;
  /** `InitializePoolParameters.liquidity` (u128). */
  liquidity: bigint | number;
  /** `InitializePoolParameters.sqrt_price` (u128, Q64.64). */
  sqrtPrice: bigint | number;
  /** `InitializePoolParameters.activation_point` (Option<u64>). Omit → None. */
  activationPoint?: bigint | number | null;
  /** Token program owning mint A (SPL Token by default). */
  tokenAProgram?: AddressInput;
  /** Token program owning mint B (SPL Token by default). */
  tokenBProgram?: AddressInput;
}

/**
 * `cp_amm::initialize_pool` — creates the Pool, its two token vaults, and the
 * FIRST position (minting a Token-2022 position NFT to `creator`), then deposits
 * `liquidity` at `sqrt_price`.
 *
 * Args (ix_initialize_pool.rs:39-47): `liquidity: u128 ++ sqrt_price: u128 ++
 * activation_point: Option<u64>`. Accounts (InitializePoolCtx, lines 51-185) +
 * `#[event_cpi]` trailer.
 */
export async function initializePool(a: InitializePoolArgs): Promise<TransactionInstruction> {
  const tokenAProgram = a.tokenAProgram ?? TOKEN_PROGRAM_ID;
  const tokenBProgram = a.tokenBProgram ?? TOKEN_PROGRAM_ID;
  const poolAddr = (await mpda.pool(a.config, a.tokenAMint, a.tokenBMint)).address;
  const positionAddr = (await mpda.position(a.positionNftMint)).address;
  const positionNftAccount = (await mpda.positionNftAccount(a.positionNftMint)).address;
  const tokenAVault = (await mpda.tokenVault(a.tokenAMint, poolAddr)).address;
  const tokenBVault = (await mpda.tokenVault(a.tokenBMint, poolAddr)).address;
  const poolAuthority = (await mpda.poolAuthority()).address;
  const eventAuthority = (await mpda.eventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(METEORA_DAMM_V2_ID),
    keys: [
      ro(a.creator),
      w(a.positionNftMint, true),
      w(positionNftAccount),
      w(a.payer, true),
      ro(a.config),
      ro(poolAuthority),
      w(poolAddr),
      w(positionAddr),
      ro(a.tokenAMint),
      ro(a.tokenBMint),
      w(tokenAVault),
      w(tokenBVault),
      w(a.payerTokenA),
      w(a.payerTokenB),
      ro(tokenAProgram),
      ro(tokenBProgram),
      ro(TOKEN_2022_PROGRAM_ID),
      ro(SYSTEM_PROGRAM_ID),
      ro(eventAuthority),
      ro(METEORA_DAMM_V2_ID),
    ],
    data: concat([
      DISC.initializePool,
      u128le(a.liquidity),
      u128le(a.sqrtPrice),
      optionU64le(a.activationPoint),
    ]),
  });
}

export interface CreatePositionArgs {
  /** Position-NFT recipient (UncheckedAccount, not a signer). */
  owner: AddressInput;
  /** New position-NFT mint — a fresh keypair the caller must also sign with. */
  positionNftMint: AddressInput;
  /** The Pool to open a position in. */
  pool: AddressInput;
  /** Fee/rent payer + signer. */
  payer: AddressInput;
}

/**
 * `cp_amm::create_position` — opens an EMPTY position (zero liquidity), minting a
 * Token-2022 position NFT to `owner`. No args.
 *
 * Accounts (CreatePositionCtx, ix_create_position.rs:411-469) + `#[event_cpi]`.
 */
export async function createPosition(a: CreatePositionArgs): Promise<TransactionInstruction> {
  const positionAddr = (await mpda.position(a.positionNftMint)).address;
  const positionNftAccount = (await mpda.positionNftAccount(a.positionNftMint)).address;
  const poolAuthority = (await mpda.poolAuthority()).address;
  const eventAuthority = (await mpda.eventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(METEORA_DAMM_V2_ID),
    keys: [
      ro(a.owner),
      w(a.positionNftMint, true),
      w(positionNftAccount),
      w(a.pool),
      w(positionAddr),
      ro(poolAuthority),
      w(a.payer, true),
      ro(TOKEN_2022_PROGRAM_ID),
      ro(SYSTEM_PROGRAM_ID),
      ro(eventAuthority),
      ro(METEORA_DAMM_V2_ID),
    ],
    data: DISC.createPosition,
  });
}
