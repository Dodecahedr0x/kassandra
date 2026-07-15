/**
 * Shared meta + borsh helpers for the Meteora DAMM v2 (cp-amm) instruction
 * builders. Internal to the `instructions/` folder module — NOT re-exported by
 * `./index.ts`, so they stay off the package's public surface.
 */
import { Address } from "@solana/web3.js";
import type { AccountMeta } from "@solana/web3.js";

import { concatBytes as concat, u64LE as u64le } from "../../bytes.js";
import type { AddressInput } from "../../pda.js";

export function addr(a: AddressInput): Address {
  return a instanceof Address ? a : new Address(a);
}
export function w(pubkey: AddressInput, isSigner = false): AccountMeta {
  return { pubkey: addr(pubkey), isSigner, isWritable: true };
}
export function ro(pubkey: AddressInput, isSigner = false): AccountMeta {
  return { pubkey: addr(pubkey), isSigner, isWritable: false };
}

/** Borsh `Option<u64>`: `0x00` (None) or `0x01 ++ u64le` (Some). */
export function optionU64le(v: bigint | number | null | undefined): Uint8Array {
  if (v === null || v === undefined) return Uint8Array.from([0]);
  return concat([Uint8Array.from([1]), u64le(v)]);
}
