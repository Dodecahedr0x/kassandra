import type { Address, Connection, TransactionInstruction } from "@solana/web3.js";
import type { AddressInput } from "../../actions";

/** A labelled, single-tx group of instructions in the compose→open sequence. */
export interface ComposeStep {
  /** A stable id for resume/skip logic. */
  id: string;
  /** A human label for the progress UI (e.g. "Create question"). */
  label: string;
  /** The instructions to send in ONE transaction for this step. */
  ixs: TransactionInstruction[];
  /**
   * Optional compute-unit budget hint for this step (some steps CPI heavily —
   * split_tokens / open_challenge). The UI prepends a setComputeUnitLimit ix.
   */
  computeUnits?: number;
}

/**
 * The fully-derived account set the compose produces — the question, the two
 * conditional vaults + their pass/fail mints, the two AMM pool PDAs, and the
 * oracle-owned pass/fail KASS holder ATAs. Returned alongside the steps so a
 * caller (the E2E) can assert against the on-chain accounts.
 */
export interface ComposedMarket {
  oracle: Address;
  question: Address;
  kassVault: Address;
  usdcVault: Address;
  kassVaultUnderlying: Address;
  usdcVaultUnderlying: Address;
  passKassMint: Address;
  failKassMint: Address;
  passUsdcMint: Address;
  failUsdcMint: Address;
  passAmm: Address;
  failAmm: Address;
  oraclePassKass: Address;
  oracleFailKass: Address;
  /** The challenger's USDC source account funding the escrow (its USDC ATA). */
  challengerUsdcSrc: Address;
}

export interface BuildComposeArgs {
  connection: Connection;
  /** Oracle nonce (re-derives the oracle PDA that resolves the question / signs). */
  oracleNonce: bigint | number;
  /** The challenged claim's Proposer PDA (open_challenge derives ai_claim/market). */
  proposer: AddressInput;
  /** Challenger (signer): composes + funds everything, opens the Market. */
  challenger: AddressInput;
  /** The oracle's KASS mint (`oracle.kassMint`). */
  kassMint: AddressInput;
  /** The oracle's USDC mint (`oracle.usdcMint`). */
  usdcMint: AddressInput;
  /** The futarchy `Dao` (`== protocol.kass_dao`) — kass_price source for the escrow. */
  kassDao: AddressInput;
  /**
   * 32-byte question id (seeds the Question PDA). Defaults to a deterministic
   * fill so the same challenger→oracle produces the same market. A caller may
   * pass a distinct id.
   */
  questionId?: Uint8Array;
  /** Base (conditional-KASS) reserve to seed each pool with. Default 100 KASS (9 dp). */
  baseReserve?: bigint | number;
  /** Quote (conditional-USDC) reserve to seed each pool with. Default 100 USDC (6 dp). */
  quoteReserve?: bigint | number;
  programId?: Address;
}
