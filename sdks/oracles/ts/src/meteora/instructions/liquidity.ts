/**
 * Meteora DAMM v2 (cp-amm) liquidity builders: `addLiquidity` / `removeLiquidity`
 * over an existing position (shared `ModifyLiquidityParameters` wire layout).
 */
import { TransactionInstruction } from "@solana/web3.js";

import { concatBytes as concat, u128LE as u128le, u64LE as u64le } from "../../bytes.js";
import { TOKEN_PROGRAM_ID } from "../../constants.js";
import type { AddressInput } from "../../pda.js";
import { DISC, METEORA_DAMM_V2_ID } from "../constants.js";
import * as mpda from "../pda.js";
import { addr, ro, w } from "./shared.js";

export interface ModifyLiquidityArgs {
  pool: AddressInput;
  position: AddressInput;
  /** Signer's token-A account. */
  tokenAAccount: AddressInput;
  /** Signer's token-B account. */
  tokenBAccount: AddressInput;
  tokenAVault: AddressInput;
  tokenBVault: AddressInput;
  tokenAMint: AddressInput;
  tokenBMint: AddressInput;
  /** Token account holding the position NFT (proves ownership; amount == 1). */
  positionNftAccount: AddressInput;
  /** Position owner / delegate + signer. */
  signer: AddressInput;
  /** `liquidity_delta` (u128). */
  liquidityDelta: bigint | number;
  /** Token-A threshold (u64): MAX to spend on add, MIN to receive on remove. */
  tokenAAmountThreshold: bigint | number;
  /** Token-B threshold (u64): MAX to spend on add, MIN to receive on remove. */
  tokenBAmountThreshold: bigint | number;
  tokenAProgram?: AddressInput;
  tokenBProgram?: AddressInput;
}

/**
 * `cp_amm::add_liquidity` — deposits into an existing position.
 *
 * Args (AddLiquidityParameters, ix_add_liquidity.rs:575-583): `liquidity_delta:
 * u128 ++ token_a_amount_threshold: u64 ++ token_b_amount_threshold: u64` (the
 * thresholds are the MAX amounts to spend). Accounts (AddLiquidityCtx, lines
 * 587-634) + `#[event_cpi]`.
 */
export async function addLiquidity(a: ModifyLiquidityArgs): Promise<TransactionInstruction> {
  const tokenAProgram = a.tokenAProgram ?? TOKEN_PROGRAM_ID;
  const tokenBProgram = a.tokenBProgram ?? TOKEN_PROGRAM_ID;
  const eventAuthority = (await mpda.eventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(METEORA_DAMM_V2_ID),
    keys: [
      w(a.pool),
      w(a.position),
      w(a.tokenAAccount),
      w(a.tokenBAccount),
      w(a.tokenAVault),
      w(a.tokenBVault),
      ro(a.tokenAMint),
      ro(a.tokenBMint),
      ro(a.positionNftAccount),
      ro(a.signer, true),
      ro(tokenAProgram),
      ro(tokenBProgram),
      ro(eventAuthority),
      ro(METEORA_DAMM_V2_ID),
    ],
    data: concat([
      DISC.addLiquidity,
      u128le(a.liquidityDelta),
      u64le(a.tokenAAmountThreshold),
      u64le(a.tokenBAmountThreshold),
    ]),
  });
}

/**
 * `cp_amm::remove_liquidity` — withdraws from an existing position.
 *
 * Args (RemoveLiquidityParameters, ix_remove_liquidity.rs:765-773): `liquidity_delta:
 * u128 ++ token_a_amount_threshold: u64 ++ token_b_amount_threshold: u64` (the
 * thresholds are the MIN amounts to receive). Accounts (RemoveLiquidityCtx, lines
 * 777-828) — same as add_liquidity but PREFIXED with the `pool_authority` account
 * (the vault-transfer signer) — + `#[event_cpi]`.
 */
export async function removeLiquidity(a: ModifyLiquidityArgs): Promise<TransactionInstruction> {
  const tokenAProgram = a.tokenAProgram ?? TOKEN_PROGRAM_ID;
  const tokenBProgram = a.tokenBProgram ?? TOKEN_PROGRAM_ID;
  const poolAuthority = (await mpda.poolAuthority()).address;
  const eventAuthority = (await mpda.eventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(METEORA_DAMM_V2_ID),
    keys: [
      ro(poolAuthority),
      w(a.pool),
      w(a.position),
      w(a.tokenAAccount),
      w(a.tokenBAccount),
      w(a.tokenAVault),
      w(a.tokenBVault),
      ro(a.tokenAMint),
      ro(a.tokenBMint),
      ro(a.positionNftAccount),
      ro(a.signer, true),
      ro(tokenAProgram),
      ro(tokenBProgram),
      ro(eventAuthority),
      ro(METEORA_DAMM_V2_ID),
    ],
    data: concat([
      DISC.removeLiquidity,
      u128le(a.liquidityDelta),
      u64le(a.tokenAAmountThreshold),
      u64le(a.tokenBAmountThreshold),
    ]),
  });
}
