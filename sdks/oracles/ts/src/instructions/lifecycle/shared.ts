/**
 * Shared meta helpers for the protocol + oracle-lifecycle instruction builders.
 * Internal to the `lifecycle/` folder module — NOT re-exported by `./index.ts`.
 */
import { Address } from "@solana/web3.js";
import type { AccountMeta } from "@solana/web3.js";

import type { AddressInput } from "../../pda.js";

/** Coerce an `AddressInput` into a web3.js `Address`. */
export function addr(a: AddressInput): Address {
  return a instanceof Address ? a : new Address(a);
}

/** Writable account meta. */
export function w(pubkey: Address, isSigner = false): AccountMeta {
  return { pubkey, isSigner, isWritable: true };
}

/** Read-only account meta. */
export function ro(pubkey: Address, isSigner = false): AccountMeta {
  return { pubkey, isSigner, isWritable: false };
}
