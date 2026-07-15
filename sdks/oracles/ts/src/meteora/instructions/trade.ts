/**
 * Meteora DAMM v2 (cp-amm) trade + fee builders: `swap` (direction implicit in
 * the input/output token accounts) and `claimPositionFee`.
 */
import { TransactionInstruction } from "@solana/web3.js";

import { concatBytes as concat, u64LE as u64le } from "../../bytes.js";
import { TOKEN_PROGRAM_ID } from "../../constants.js";
import type { AddressInput } from "../../pda.js";
import { DISC, METEORA_DAMM_V2_ID } from "../constants.js";
import * as mpda from "../pda.js";
import { addr, ro, w } from "./shared.js";

export interface SwapArgs {
  pool: AddressInput;
  /** The trader's INPUT token account (the token being sold). */
  inputTokenAccount: AddressInput;
  /** The trader's OUTPUT token account (the token being bought). */
  outputTokenAccount: AddressInput;
  tokenAVault: AddressInput;
  tokenBVault: AddressInput;
  tokenAMint: AddressInput;
  tokenBMint: AddressInput;
  /** Trader + signer. */
  payer: AddressInput;
  /** `SwapParameters.amount_in` (u64). */
  amountIn: bigint | number;
  /** `SwapParameters.minimum_amount_out` (u64). */
  minimumAmountOut?: bigint | number;
  /** Optional referral token account; omitted → the program id (the None sentinel). */
  referralTokenAccount?: AddressInput;
  tokenAProgram?: AddressInput;
  tokenBProgram?: AddressInput;
}

/**
 * `cp_amm::swap` — trades against the pool. Direction is IMPLICIT: whichever of
 * A/B the `inputTokenAccount`/`outputTokenAccount` correspond to. There is NO
 * `swap_type` arg.
 *
 * Args (SwapParameters, swap/ix_swap.rs:986-990): `amount_in: u64 ++
 * minimum_amount_out: u64`. Accounts (SwapCtx, lines 1014-1059) + `#[event_cpi]`.
 * The `referral_token_account` is an `Option<…>`: when absent, Anchor expects the
 * program id in its slot (checked at ix_swap.rs:1156).
 */
export async function swap(a: SwapArgs): Promise<TransactionInstruction> {
  const tokenAProgram = a.tokenAProgram ?? TOKEN_PROGRAM_ID;
  const tokenBProgram = a.tokenBProgram ?? TOKEN_PROGRAM_ID;
  const poolAuthority = (await mpda.poolAuthority()).address;
  const eventAuthority = (await mpda.eventAuthority()).address;
  const referral = a.referralTokenAccount ?? METEORA_DAMM_V2_ID;
  return new TransactionInstruction({
    programId: addr(METEORA_DAMM_V2_ID),
    keys: [
      ro(poolAuthority),
      w(a.pool),
      w(a.inputTokenAccount),
      w(a.outputTokenAccount),
      w(a.tokenAVault),
      w(a.tokenBVault),
      ro(a.tokenAMint),
      ro(a.tokenBMint),
      ro(a.payer, true),
      ro(tokenAProgram),
      ro(tokenBProgram),
      a.referralTokenAccount ? w(referral) : ro(referral),
      ro(eventAuthority),
      ro(METEORA_DAMM_V2_ID),
    ],
    data: concat([DISC.swap, u64le(a.amountIn), u64le(a.minimumAmountOut ?? 0)]),
  });
}

export interface ClaimPositionFeeArgs {
  pool: AddressInput;
  position: AddressInput;
  /** Owner's token-A destination account. */
  tokenAAccount: AddressInput;
  /** Owner's token-B destination account. */
  tokenBAccount: AddressInput;
  tokenAVault: AddressInput;
  tokenBVault: AddressInput;
  tokenAMint: AddressInput;
  tokenBMint: AddressInput;
  positionNftAccount: AddressInput;
  /** Position owner / delegate + signer. */
  signer: AddressInput;
  tokenAProgram?: AddressInput;
  tokenBProgram?: AddressInput;
}

/**
 * `cp_amm::claim_position_fee` — sweeps a position's pending fees to the owner.
 * No args. NOTE the `pool` account is READ-ONLY here (fees live on the Position).
 *
 * Accounts (ClaimPositionFeeCtx, ix_claim_position_fee.rs:1176-1233) + `#[event_cpi]`.
 */
export async function claimPositionFee(a: ClaimPositionFeeArgs): Promise<TransactionInstruction> {
  const tokenAProgram = a.tokenAProgram ?? TOKEN_PROGRAM_ID;
  const tokenBProgram = a.tokenBProgram ?? TOKEN_PROGRAM_ID;
  const poolAuthority = (await mpda.poolAuthority()).address;
  const eventAuthority = (await mpda.eventAuthority()).address;
  return new TransactionInstruction({
    programId: addr(METEORA_DAMM_V2_ID),
    keys: [
      ro(poolAuthority),
      ro(a.pool),
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
    data: DISC.claimPositionFee,
  });
}
