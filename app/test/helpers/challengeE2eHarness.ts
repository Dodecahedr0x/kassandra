/**
 * RF4 forked-mainnet challenge E2E — harness primitives: the {@link Fixture},
 * shared constants, dispute-core RPC drivers, and the MetaDAO market-composition
 * + v0.4 AMM builders (ported from challenge-market-e2e). Pure move/extract from
 * `challenge.e2e.test.ts` — no behavior change. The app-seam drivers that consume
 * these live in `challengeE2eDrivers.ts`.
 */
import {
  Address,
  ComputeBudgetProgram,
  Keypair,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  EXTERNAL_PROGRAM_IDS,
  ammV04,
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

export const SYSTEM_PROGRAM_ID = new Address("11111111111111111111111111111111");
export const ATA_PROGRAM_ID = new Address("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
export const VLTX = EXTERNAL_PROGRAM_IDS.conditionalVault;

export const INITIALIZE_QUESTION = Uint8Array.from([0xf5, 0x97, 0x6a, 0xbc, 0x58, 0x2c, 0x41, 0xd4]);
export const INITIALIZE_CONDITIONAL_VAULT = Uint8Array.from([0x25, 0x58, 0xfa, 0xd4, 0x36, 0xda, 0xe3, 0xaf]);

export const enc = new TextEncoder();

export const BOND = 1_000_000_000n;
export const MAX_PRICE = ((1n << 64n) - 1n) * 1_000_000_000_000n;

export interface Fixture {
  harness: SurfpoolHarness;
  payer: Keypair;
  kassMint: Keypair;
  usdcMint: Keypair;
  kassDao: Address;
}

export interface VaultAccounts {
  vault: Address;
  underlying: Address;
  passMint: Address;
  failMint: Address;
}

// ---------------------------------------------------------------------------
// MetaDAO market composition + AMM driving (ported from challenge-market-e2e).
// ---------------------------------------------------------------------------
export async function ata(owner: Address, mint: Address): Promise<Address> {
  return (
    await Address.findProgramAddress(
      [owner.toBytes(), TOKEN_PROGRAM_ID.toBytes(), mint.toBytes()],
      ATA_PROGRAM_ID,
    )
  )[0];
}

export async function composeQuestion(
  f: Fixture,
  resolver: Address,
  questionId: Uint8Array,
  numOutcomes: number,
): Promise<{ question: Address }> {
  const [question] = await Address.findProgramAddress(
    [enc.encode("question"), questionId, resolver.toBytes(), Uint8Array.from([numOutcomes])],
    VLTX,
  );
  const [eventAuthority] = await Address.findProgramAddress([enc.encode("__event_authority")], VLTX);

  const data = new Uint8Array(73);
  data.set(INITIALIZE_QUESTION, 0);
  data.set(questionId, 8);
  data.set(resolver.toBytes(), 40);
  data[72] = numOutcomes;

  await sendIx(
    f,
    new TransactionInstruction({
      programId: VLTX,
      keys: [
        { pubkey: question, isSigner: false, isWritable: true },
        { pubkey: f.payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: eventAuthority, isSigner: false, isWritable: false },
        { pubkey: VLTX, isSigner: false, isWritable: false },
      ],
      data,
    }),
    [],
    400_000,
  );
  return { question };
}

export async function composeVault(f: Fixture, question: Address, underlyingMint: Address): Promise<VaultAccounts> {
  const [vault] = await Address.findProgramAddress(
    [enc.encode("conditional_vault"), question.toBytes(), underlyingMint.toBytes()],
    VLTX,
  );
  const [passMint] = await Address.findProgramAddress(
    [enc.encode("conditional_token"), vault.toBytes(), Uint8Array.from([0])],
    VLTX,
  );
  const [failMint] = await Address.findProgramAddress(
    [enc.encode("conditional_token"), vault.toBytes(), Uint8Array.from([1])],
    VLTX,
  );
  const [eventAuthority] = await Address.findProgramAddress([enc.encode("__event_authority")], VLTX);
  const underlying = await ata(vault, underlyingMint);

  await sendIx(
    f,
    new TransactionInstruction({
      programId: VLTX,
      keys: [
        { pubkey: vault, isSigner: false, isWritable: true },
        { pubkey: question, isSigner: false, isWritable: false },
        { pubkey: underlyingMint, isSigner: false, isWritable: false },
        { pubkey: underlying, isSigner: false, isWritable: true },
        { pubkey: f.payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ATA_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: eventAuthority, isSigner: false, isWritable: false },
        { pubkey: VLTX, isSigner: false, isWritable: false },
        { pubkey: passMint, isSigner: false, isWritable: true },
        { pubkey: failMint, isSigner: false, isWritable: true },
      ],
      data: INITIALIZE_CONDITIONAL_VAULT,
    }),
    [],
    400_000,
  );
  return { vault, underlying, passMint, failMint };
}

export async function fabricateTokenAccountMint(
  f: Fixture,
  mint: Address,
  owner: Address,
  amount: bigint,
): Promise<Address> {
  const acct = await Keypair.generate();
  await f.harness.setAccount(acct.publicKey.toString(), {
    lamports: 5_000_000,
    owner: TOKEN_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(tokenAccountBytes(mint.toBytes(), owner.toBytes(), amount)),
  });
  return acct.publicKey;
}

export async function setTokenAccountAt(
  f: Fixture,
  address: Address,
  mint: Address,
  owner: Address,
  amount: bigint,
): Promise<void> {
  await f.harness.setAccount(address.toString(), {
    lamports: 5_000_000,
    owner: TOKEN_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(tokenAccountBytes(mint.toBytes(), owner.toBytes(), amount)),
  });
}

async function advanceSlots(f: Fixture, n: number): Promise<void> {
  const start = await f.harness.currentSlot();
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if ((await f.harness.currentSlot()) >= start + n) return;
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error(`slot did not advance by ${n} within 30s (clock mode not producing blocks?)`);
}

export async function buildPool(
  f: Fixture,
  baseMint: Address,
  quoteMint: Address,
  baseReserve: bigint,
  quoteReserve: bigint,
): Promise<Address> {
  const ammAddr = (await ammV04.pda.amm(baseMint, quoteMint)).address;
  const lp = (await ammV04.pda.lpMint(ammAddr)).address;
  const userBase = await ammV04.pda.ata(f.payer.publicKey, baseMint);
  const userQuote = await ammV04.pda.ata(f.payer.publicKey, quoteMint);
  await setTokenAccountAt(f, userBase, baseMint, f.payer.publicKey, baseReserve * 4n);
  await setTokenAccountAt(f, userQuote, quoteMint, f.payer.publicKey, quoteReserve * 4n);

  const initialObs = (quoteReserve * 1_000_000_000_000n) / baseReserve;
  await sendIx(
    f,
    await ammV04.createAmm({
      payer: f.payer.publicKey,
      baseMint,
      quoteMint,
      twapInitialObservation: initialObs,
      twapMaxObservationChangePerUpdate: MAX_PRICE,
      twapStartDelaySlots: 0n,
    }),
    [],
    1_400_000,
  );

  const userLp = await ammV04.pda.ata(f.payer.publicKey, lp);
  await setTokenAccountAt(f, userLp, lp, f.payer.publicKey, 0n);
  await sendIx(
    f,
    await ammV04.addLiquidity({
      payer: f.payer.publicKey,
      baseMint,
      quoteMint,
      quoteAmount: quoteReserve,
      maxBaseAmount: baseReserve,
      minLpTokens: 0n,
    }),
    [],
    1_400_000,
  );
  return ammAddr;
}

export async function swapBuy(f: Fixture, baseMint: Address, quoteMint: Address, amountIn: bigint): Promise<void> {
  await advanceSlots(f, 200);
  await sendIx(
    f,
    await ammV04.swap({
      payer: f.payer.publicKey,
      baseMint,
      quoteMint,
      swapType: ammV04.SwapType.Buy,
      inputAmount: amountIn,
      minOutputAmount: 0n,
    }),
    [],
    1_400_000,
  );
}

export async function crankPool(f: Fixture, amm: Address): Promise<void> {
  await advanceSlots(f, 300);
  await sendIx(f, await ammV04.crankThatTwap({ amm }), [], 400_000);
}

export function decodeAmmTwap(data: Uint8Array): { twap: bigint } {
  const dv = new DataView(data.buffer, data.byteOffset, data.length);
  const u128 = (off: number): bigint => dv.getBigUint64(off, true) | (dv.getBigUint64(off + 8, true) << 64n);
  const createdAt = dv.getBigUint64(9, true);
  const lastUpdated = dv.getBigUint64(131, true);
  const aggregator = u128(171);
  const startDelay = dv.getBigUint64(219, true);
  const slots = lastUpdated - (createdAt + startDelay);
  const twap = slots > 0n && aggregator > 0n ? aggregator / slots : 0n;
  return { twap };
}

export function questionResolution(data: Uint8Array): [number, number] {
  const dv = new DataView(data.buffer, data.byteOffset, data.length);
  return [dv.getUint32(76, true), dv.getUint32(80, true)];
}

// ---------------------------------------------------------------------------
// Dispute-core drivers over RPC (self-contained; mirror challenge-market-e2e).
// ---------------------------------------------------------------------------
export async function sendIx(
  f: Fixture,
  ix: TransactionInstruction,
  signers: Keypair[] = [],
  computeUnits?: number,
): Promise<void> {
  const conn = f.harness.connection;
  const tx = new Transaction();
  tx.feePayer = f.payer.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash;
  if (computeUnits) tx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: computeUnits }));
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

export async function tokenBalance(f: Fixture, address: Address): Promise<bigint> {
  return tokenAccountAmount(await fetchAccount(f, address));
}

export async function fundKass(f: Fixture, owner: Address, amount: bigint): Promise<Address> {
  return fabricateTokenAccountMint(f, f.kassMint.publicKey, owner, amount);
}

export async function createOracleReal(f: Fixture, nonce: bigint, optionsCount: number): Promise<void> {
  const creatorKass = await fundKass(f, f.payer.publicKey, 10n ** 15n);
  const nowUnix = await f.harness.clockUnixTimestamp();
  await sendIx(
    f,
    await createOracle({
      nonce,
      optionsCount,
      deadline: nowUnix + 1_000n,
      twapWindow: 600n,
      creator: f.payer.publicKey,
      creatorKassToken: creatorKass,
      kassMint: f.kassMint.publicKey,
      usdcMint: f.usdcMint.publicKey,
    }),
  );
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
  f: Fixture,
  oracle: Address,
  option: number,
  bond: bigint,
): Promise<{ authority: Keypair; proposer: Address }> {
  const authority = await Keypair.generate();
  await f.harness.airdrop(authority.publicKey.toString(), 2_000_000_000);
  const authorityKass = await fundKass(f, authority.publicKey, bond * 10n);
  await sendIx(f, await propose({ oracle, authority: authority.publicKey, authorityKass, option, bond }), [authority]);
  const proposer = (await pda.proposer(oracle, authority.publicKey)).address;
  return { authority, proposer };
}
