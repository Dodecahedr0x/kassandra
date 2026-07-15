/**
 * Oracle-setup lifecycle builders: `initProtocol` (Ix=9), `createOracle`
 * (Ix=10), `writeOracleMeta` (Ix=23). See `../lifecycle` for conventions.
 */
import { Address, TransactionInstruction } from "@solana/web3.js";

import {
  Ix,
  KASSANDRA_PROGRAM_ID,
  SYSTEM_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from "../../constants.js";
import * as pda from "../../pda.js";
import type { AddressInput } from "../../pda.js";
import { fixedBytes, i64LE, u16LE, u64LE, u8, withDisc } from "../payload.js";
import { addr, ro, w } from "./shared.js";

// ---------------------------------------------------------------------------
// InitProtocol (Ix=9) — processor/init_protocol.rs
// Accounts: 0 protocol(w) 1 admin(w,signer) 2 kass_mint(ro) 3 usdc_mint(ro)
//           4 system program(ro). Payload: none.
// ---------------------------------------------------------------------------
export interface InitProtocolArgs {
  /** Admin (signer): tops up rent, recorded as `Protocol.admin`. */
  admin: AddressInput;
  /** Canonical KASS mint (SPL token-program owned). */
  kassMint: AddressInput;
  /** Canonical USDC mint (SPL token-program owned). */
  usdcMint: AddressInput;
  /** Override the program id (defaults to {@link KASSANDRA_PROGRAM_ID}). */
  programId?: Address;
}

export async function initProtocol(args: InitProtocolArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  const protocol = await pda.protocol(programId);
  return new TransactionInstruction({
    programId,
    keys: [
      w(protocol.address),
      w(addr(args.admin), true),
      ro(addr(args.kassMint)),
      ro(addr(args.usdcMint)),
      ro(SYSTEM_PROGRAM_ID),
    ],
    data: withDisc(Ix.InitProtocol),
  });
}

// ---------------------------------------------------------------------------
// CreateOracle (Ix=10) — processor/create_oracle.rs
// Accounts: 0 protocol(w) 1 oracle(w,PDA) 2 stake_vault(w,PDA) 3 creator(w,signer)
//           4 kass_mint(w) 5 usdc_mint(ro) 6 token program(ro) 7 system program(ro)
//           8 creator_kass_token(w) 9 mint_authority(ro,PDA).
// Payload (57): nonce u64 ++ prompt_hash[32] ++ options_count u8 ++ deadline i64
//               ++ twap_window i64.
// ---------------------------------------------------------------------------
export interface CreateOracleArgs {
  /** Oracle nonce — seeds the oracle PDA `[b"oracle", nonce_le8]`. */
  nonce: bigint | number;
  /** Categorical option count (>= 2). */
  optionsCount: number;
  /** Creation-time deadline (unix seconds, i64). */
  deadline: bigint | number;
  /** TWAP window (seconds, i64 > 0). */
  twapWindow: bigint | number;
  /** Creator (signer): pays rent, recorded as creator, fee-burn authority. */
  creator: AddressInput;
  /** Creator's KASS token account the creation fee is burned from. */
  creatorKassToken: AddressInput;
  /** Canonical KASS mint (must equal `protocol.kass_mint`). */
  kassMint: AddressInput;
  /** Canonical USDC mint (must equal `protocol.usdc_mint`). */
  usdcMint: AddressInput;
  programId?: Address;
}

export async function createOracle(args: CreateOracleArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  const protocol = await pda.protocol(programId);
  const oracle = await pda.oracle(BigInt(args.nonce), programId);
  const stakeVault = await pda.stakeVault(oracle.address, programId);
  const mintAuthority = await pda.mintAuthority(programId);

  const data = withDisc(
    Ix.CreateOracle,
    u64LE(args.nonce),
    u8(args.optionsCount),
    i64LE(args.deadline),
    i64LE(args.twapWindow),
  );

  return new TransactionInstruction({
    programId,
    keys: [
      w(protocol.address),
      w(oracle.address),
      w(stakeVault.address),
      w(addr(args.creator), true),
      w(addr(args.kassMint)),
      ro(addr(args.usdcMint)),
      ro(TOKEN_PROGRAM_ID),
      ro(SYSTEM_PROGRAM_ID),
      w(addr(args.creatorKassToken)),
      ro(mintAuthority.address),
    ],
    data,
  });
}

// ---------------------------------------------------------------------------
// WriteOracleMeta (Ix=23) — processor/write_oracle_meta.rs
// Accounts: 0 creator(w,signer) 1 oracle(ro) 2 oracle_meta(w,PDA) 3 system(ro).
// Body (length-prefixed): subject_len u16 ++ subject ++ options_count u8 ++
//   [option_len u16 ++ option]* ++ uri_len u16 ++ uri ++ uri_hash[32].
// ---------------------------------------------------------------------------
export interface WriteOracleMetaArgs {
  /** The oracle whose metadata is being written. */
  oracle: AddressInput;
  /** Creator (signer): must equal the oracle's recorded creator; pays the rent. */
  creator: AddressInput;
  /** The plaintext question (on-chain). */
  subject: string;
  /** The option labels (on-chain); count must equal the oracle's options_count. */
  options: string[];
  /** URL of the extended off-chain metadata JSON (may be empty). */
  uri: string;
  /** 32-byte `sha256` of the canonical off-chain JSON (zeroed if no uri). */
  uriHash: Uint8Array;
  programId?: Address;
}

export async function writeOracleMeta(
  args: WriteOracleMetaArgs,
): Promise<TransactionInstruction> {
  const programId = args.programId ?? KASSANDRA_PROGRAM_ID;
  const meta = await pda.oracleMeta(addr(args.oracle), programId);

  const enc = new TextEncoder();
  const subject = enc.encode(args.subject);
  const parts: Uint8Array[] = [u16LE(subject.length), subject, u8(args.options.length)];
  for (const o of args.options) {
    const b = enc.encode(o);
    parts.push(u16LE(b.length), b);
  }
  const uri = enc.encode(args.uri);
  parts.push(u16LE(uri.length), uri, fixedBytes(args.uriHash, 32));

  return new TransactionInstruction({
    programId,
    keys: [
      w(addr(args.creator), true),
      ro(addr(args.oracle)),
      w(meta.address),
      ro(SYSTEM_PROGRAM_ID),
    ],
    data: withDisc(Ix.WriteOracleMeta, ...parts),
  });
}
