/**
 * Shared meta + borsh helpers for the futarchy v0.6 + Squads v4 +
 * conditional_vault instruction builders.
 *
 * `ATA_PROGRAM_ID` and `ata` are part of the public surface (re-exported by
 * `./index.ts`); the low-level meta/borsh helpers are internal to the folder
 * module and NOT re-exported.
 */
import { Address } from "@solana/web3.js";
import type { AccountMeta } from "@solana/web3.js";

import { concatBytes as concat, u32LE as u32le } from "../../bytes.js";
import { TOKEN_PROGRAM_ID } from "../../constants.js";
import type { AddressInput } from "../../pda.js";

/** The SPL Associated Token Account program. */
export const ATA_PROGRAM_ID = new Address("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

export function addr(a: AddressInput): Address {
  return a instanceof Address ? a : new Address(a);
}
export function w(pubkey: AddressInput, isSigner = false): AccountMeta {
  return { pubkey: addr(pubkey), isSigner, isWritable: true };
}
export function ro(pubkey: AddressInput, isSigner = false): AccountMeta {
  return { pubkey: addr(pubkey), isSigner, isWritable: false };
}

export function boolb(v: boolean): Uint8Array {
  return Uint8Array.from([v ? 1 : 0]);
}
/** Borsh `Vec<u8>` — u32 LE length prefix then the bytes. */
export function vecU8(bytes: Uint8Array): Uint8Array {
  return concat([u32le(bytes.length), bytes]);
}
/** Borsh `Option<String>` (None or UTF-8 with a u32 length prefix). */
export function optString(s: string | null | undefined): Uint8Array {
  if (s === null || s === undefined) return Uint8Array.from([0]);
  const b = new TextEncoder().encode(s);
  return concat([Uint8Array.from([1]), u32le(b.length), b]);
}

/** Associated token account `[owner, TOKEN_PROGRAM, mint]` under the ATA program. */
export async function ata(owner: AddressInput, mint: AddressInput): Promise<Address> {
  const [a] = await Address.findProgramAddress(
    [addr(owner).toBytes(), TOKEN_PROGRAM_ID.toBytes(), addr(mint).toBytes()],
    ATA_PROGRAM_ID,
  );
  return a;
}
