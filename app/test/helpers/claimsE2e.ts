/**
 * Fixture + reward math + SEED/real-instruction RPC drivers for the RF2
 * claim/close/sweep E2E (`claims.e2e.test.ts`), ported from settlement-e2e.
 * Pure move/extract from the test file — no behavior change.
 */
import { Address, Keypair, Transaction, type TransactionInstruction } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createOracle,
  decodeOracle,
  propose,
} from "@kassandra-market/oracles";
import * as pda from "@kassandra-market/oracles";

import {
  SurfpoolHarness,
  toHex,
  tokenAccountAmount,
  tokenAccountBytes,
} from "../../../sdks/oracles/ts/test/surfpool/harness.ts";

export interface Fixture {
  harness: SurfpoolHarness;
  payer: Keypair;
  kassMint: Keypair;
  usdcMint: Keypair;
  daoAuthority: Address;
  treasury: Address;
}

// ---------------------------------------------------------------------------
// Reward math (mirrors reward.rs / claims.rs; ported from settlement-e2e).
// ---------------------------------------------------------------------------
export function rewardBuckets(pool: bigint, pw: bigint, fw: bigint, tcp: bigint, taf: bigint): [bigint, bigint] {
  if (taf === 0n) return [pool, 0n];
  if (tcp === 0n) return [0n, pool];
  const denom = pw + fw;
  if (denom === 0n) return [pool, 0n];
  return [(pool * pw) / denom, (pool * fw) / denom];
}
export function proposerReward(bond: bigint, bucket: bigint, tcp: bigint): bigint {
  return tcp === 0n ? 0n : (bond * bucket) / tcp;
}
export function factReward(stake: bigint, bucket: bigint, taf: bigint): bigint {
  return taf === 0n ? 0n : (stake * bucket) / taf;
}
export function ceilSlash(value: bigint, num: bigint, den: bigint): bigint {
  return den === 0n ? 0n : (value * num + den - 1n) / den;
}

// ---------------------------------------------------------------------------
// SEED + real-instruction drivers over RPC (ported from settlement-e2e).
// ---------------------------------------------------------------------------
export function marketBytes(oracle: Address, challenger: Address, escrow: Address): Uint8Array {
  const d = new Uint8Array(416);
  d[0] = 6; // AccountType.Market
  d.set(oracle.toBytes(), 8);
  d.set(challenger.toBytes(), 104);
  d.set(escrow.toBytes(), 360);
  d[408] = 1; // settled
  return d;
}

export async function sendIx(f: Fixture, ix: TransactionInstruction, signers: Keypair[] = []): Promise<void> {
  const conn = f.harness.connection;
  const tx = new Transaction();
  tx.feePayer = f.payer.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash;
  tx.add(ix);
  await tx.sign(f.payer, ...signers);
  const sig = await conn.sendRawTransaction(await tx.serialize(), { skipPreflight: false });
  await f.harness.confirmSignature(sig);
}

export async function fetchAccount(f: Fixture, address: Address, timeoutMs = 20_000): Promise<Uint8Array> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const info = await f.harness.connection.getAccountInfo(address);
    if (info && info.data.length > 0) return info.data;
    await new Promise((r) => setTimeout(r, 150));
  }
  throw new Error(`account ${address} did not appear within ${timeoutMs}ms`);
}

export async function isClosed(f: Fixture, address: Address): Promise<boolean> {
  const info = await f.harness.connection.getAccountInfo(address);
  return info === null || info.data.length === 0;
}

export async function tokenBalance(f: Fixture, address: Address): Promise<bigint> {
  return tokenAccountAmount(await fetchAccount(f, address));
}

export async function fundSigner(f: Fixture): Promise<Keypair> {
  const kp = await Keypair.generate();
  await f.harness.airdrop(kp.publicKey.toString(), 2_000_000_000);
  return kp;
}

export async function fundKass(f: Fixture, owner: Address, amount: bigint): Promise<Address> {
  const acct = await Keypair.generate();
  await f.harness.setAccount(acct.publicKey.toString(), {
    lamports: 5_000_000, owner: TOKEN_PROGRAM_ID.toString(), executable: false,
    data: toHex(tokenAccountBytes(f.kassMint.publicKey.toBytes(), owner.toBytes(), amount)),
  });
  return acct.publicKey;
}

export async function createOracleReal(f: Fixture, nonce: bigint, optionsCount: number): Promise<void> {
  const creatorKass = await fundKass(f, f.payer.publicKey, 10n ** 15n);
  const nowUnix = await f.harness.clockUnixTimestamp();
  await sendIx(f, await createOracle({
    nonce, optionsCount,
    deadline: nowUnix + 1_000n, twapWindow: 600n,
    creator: f.payer.publicKey, creatorKassToken: creatorKass,
    kassMint: f.kassMint.publicKey, usdcMint: f.usdcMint.publicKey,
  }));
}

export async function openProposals(f: Fixture, oracle: Address): Promise<void> {
  const o = decodeOracle(await fetchAccount(f, oracle));
  await f.harness.advanceToUnix(o.deadline + 60n);
}

export async function advancePastPhaseEnd(f: Fixture, oracle: Address): Promise<void> {
  const o = decodeOracle(await fetchAccount(f, oracle));
  await f.harness.advanceToUnix(o.phaseEndsAt + 120n);
}

export async function proposeRealWithAuthority(
  f: Fixture, oracle: Address, option: number, bond: bigint,
): Promise<{ authority: Keypair; proposer: Address }> {
  const authority = await fundSigner(f);
  const authorityKass = await fundKass(f, authority.publicKey, bond * 10n);
  await sendIx(f, await propose({ oracle, authority: authority.publicKey, authorityKass, option, bond }), [authority]);
  return { authority, proposer: (await pda.proposer(oracle, authority.publicKey)).address };
}
