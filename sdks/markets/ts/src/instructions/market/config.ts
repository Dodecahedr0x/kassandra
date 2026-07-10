/**
 * Config-singleton instruction builders (Ix 0–1).
 *
 * See `../market/index.js` for the module overview. Account orders + payload
 * layouts are mirrored VERBATIM from the verified Rust builders in
 * `sdks/oracles/rust/src/ix.rs` (a mismatch is a silent runtime failure).
 */
import { Address, TransactionInstruction } from "@solana/web3.js";

import { Ix, MARKET_PROGRAM_ID, SYSTEM_PROGRAM_ID } from "../../constants.js";
import * as pda from "../../pda.js";
import type { AddressInput } from "../../pda.js";
import { addr, pubkeyBytes, ro, u16LE, u64LE, w, withDisc } from "../payload.js";

// ---------------------------------------------------------------------------
// InitConfig (Ix 0) — create the Config singleton at PDA [b"config"].
// Payload = authority(32) ++ min_liquidity(u64 LE) ++ fee_bps(u16 LE) ++ fee_destination(32).
// Accounts: 0 config(w,PDA) 1 payer(signer,w) 2 kass_mint(ro) 3 fee_destination(ro)
//           4 system program(ro) 5 program_data(ro).
// `program_data` is this program's BPF-Upgradeable-Loader ProgramData account
// (derived from the program id): the processor reads its stored upgrade_authority
// and REQUIRES it equals `payer` (the bootstrap front-run defense).
// ---------------------------------------------------------------------------
export interface InitConfigArgs {
  /** Payer (signer): tops up rent for the Config PDA. */
  payer: AddressInput;
  /** Canonical KASS mint recorded on the Config. */
  kassMint: AddressInput;
  /** Futarchy authority recorded as `Config.authority` (payload pubkey, not an account). */
  authority: AddressInput;
  /** Minimum KASS a market must raise before activation. */
  minLiquidity: bigint | number;
  /** Protocol fee in basis points (<= {@link MAX_FEE_BPS}). */
  feeBps: number;
  /** KASS token account (on `kassMint`) protocol fees route to. */
  feeDestination: AddressInput;
  /** Override the program id (defaults to {@link MARKET_PROGRAM_ID}). */
  programId?: Address;
}

export async function initConfig(args: InitConfigArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  const config = await pda.config(programId);
  const programData = await pda.programData(programId);
  return new TransactionInstruction({
    programId,
    keys: [
      w(config.address),
      w(addr(args.payer), true),
      ro(addr(args.kassMint)),
      ro(addr(args.feeDestination)),
      ro(SYSTEM_PROGRAM_ID),
      ro(programData.address),
    ],
    data: withDisc(
      Ix.InitConfig,
      pubkeyBytes(args.authority),
      u64LE(args.minLiquidity),
      u16LE(args.feeBps),
      pubkeyBytes(args.feeDestination),
    ),
  });
}

// ---------------------------------------------------------------------------
// UpdateConfig (Ix 1) — futarchy-gated update of min_liquidity + fee_bps + fee_destination.
// Payload = min_liquidity(u64 LE) ++ fee_bps(u16 LE) ++ fee_destination(32).
// Accounts: 0 config(w) 1 authority(ro,signer) 2 fee_destination(ro).
// ---------------------------------------------------------------------------
export interface UpdateConfigArgs {
  /** Config authority (signer): must equal `Config.authority`. */
  authority: AddressInput;
  /** New minimum KASS a market must raise before activation. */
  minLiquidity: bigint | number;
  /** New protocol fee in basis points (<= {@link MAX_FEE_BPS}). */
  feeBps: number;
  /** New KASS token account (on the config's KASS mint) protocol fees route to. */
  feeDestination: AddressInput;
  programId?: Address;
}

export async function updateConfig(args: UpdateConfigArgs): Promise<TransactionInstruction> {
  const programId = args.programId ?? MARKET_PROGRAM_ID;
  const config = await pda.config(programId);
  return new TransactionInstruction({
    programId,
    keys: [w(config.address), ro(addr(args.authority), true), ro(addr(args.feeDestination))],
    data: withDisc(
      Ix.UpdateConfig,
      u64LE(args.minLiquidity),
      u16LE(args.feeBps),
      pubkeyBytes(args.feeDestination),
    ),
  });
}
