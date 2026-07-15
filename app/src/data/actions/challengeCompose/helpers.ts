import { Address, TransactionInstruction } from "@solana/web3.js";
import { ATA_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID } from "@kassandra-market/oracles";

import { ValidationError, type AddressInput } from "../../actions";

/** Coerce an {@link AddressInput}, re-typing a parse failure as a field error. */
export function addr(field: string, a: AddressInput): Address {
  if (a instanceof Address) return a;
  try {
    return new Address(a);
  } catch {
    throw new ValidationError(field, `${field} is not a valid base58 address.`);
  }
}

export function requirePositive(field: string, v: bigint): bigint {
  if (v <= 0n) throw new ValidationError(field, `${field} must be greater than zero.`);
  return v;
}

export function toBig(field: string, v: bigint | number): bigint {
  const b = typeof v === "bigint" ? v : BigInt(Math.trunc(v));
  return requirePositive(field, b);
}

/**
 * The idempotent `createAssociatedTokenAccountIdempotent` ix (ATA program
 * discriminant `1`) — the same hand-built layout the WF1 write layer uses (no
 * `@solana/spl-token` dep). Accounts: payer(w,signer), ata(w), owner(ro),
 * mint(ro), system program(ro), token program(ro).
 */
export function createAtaIdempotentIx(
  payer: Address,
  ataAddr: Address,
  owner: Address,
  mint: Address,
): TransactionInstruction {
  return new TransactionInstruction({
    programId: ATA_PROGRAM_ID,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: ataAddr, isSigner: false, isWritable: true },
      { pubkey: owner, isSigner: false, isWritable: false },
      { pubkey: mint, isSigner: false, isWritable: false },
      { pubkey: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    ],
    data: Uint8Array.of(1),
  });
}
