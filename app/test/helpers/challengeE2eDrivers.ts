/**
 * RF4 forked-mainnet challenge E2E — app-seam drivers: build via the app builders
 * ({@link buildSubmitAiClaimIxs}/{@link buildOpenChallengeIxs}/{@link
 * buildSettleFromMarketIxs}), send via keypairSender/sendAndConfirm, plus the
 * front-door + market-composition orchestration. Pure move/extract from
 * `challenge.e2e.test.ts` — no behavior change. Consumes the primitives in
 * `challengeE2eHarness.ts`.
 */
import {
  Address,
  ComputeBudgetProgram,
  Keypair,
  TransactionInstruction,
} from "@solana/web3.js";
import { expect } from "vitest";
import {
  VOTE_APPROVE,
  advancePhase,
  decodeMarket,
  decodeOracle,
  finalizeAiClaims,
  finalizeFacts,
  finalizeProposals,
  submitFact,
  voteFact,
} from "@kassandra-market/oracles";
import * as pda from "@kassandra-market/oracles";

import {
  buildOpenChallengeIxs,
  buildSubmitAiClaimIxs,
} from "../../src/data/actions/challenge.ts";
import { buildSettleFromMarketIxs } from "../../src/data/actions/challengeSettle.ts";
import { keypairSender, sendAndConfirm } from "../../src/data/send.ts";
import {
  BOND,
  type Fixture,
  type VaultAccounts,
  VLTX,
  advancePastPhaseEnd,
  ata,
  composeQuestion,
  composeVault,
  createOracleReal,
  enc,
  fabricateTokenAccountMint,
  fetchAccount,
  fundKass,
  openProposals,
  proposeRealWithAuthority,
  sendIx,
  setTokenAccountAt,
} from "./challengeE2eHarness.ts";

// ---------------------------------------------------------------------------
// App-seam senders: build via the app builders, send via keypairSender/sendAndConfirm.
// ---------------------------------------------------------------------------

/** Send app-built ixs through the app seam (optionally prepending a CU budget). */
export async function sendViaApp(
  f: Fixture,
  signer: Keypair,
  ixs: TransactionInstruction[],
  computeUnits?: number,
): Promise<void> {
  const conn = f.harness.connection;
  const withCu = computeUnits
    ? [ComputeBudgetProgram.setComputeUnitLimit({ units: computeUnits }), ...ixs]
    : ixs;
  await sendAndConfirm(conn, keypairSender(conn, signer), withCu);
}

export interface Challenged {
  oracle: Address;
  proposer: Address;
  proposerAuthority: Address;
  aiClaim: Address;
  proposerPdas: Address[];
  authorities: Keypair[];
}

export async function frontDoorToChallenge(f: Fixture, nonce: bigint): Promise<Challenged> {
  const oracle = (await pda.oracle(nonce)).address;
  const aiOption = 0;

  await createOracleReal(f, nonce, 2);
  await openProposals(f, oracle);

  const authorities: Keypair[] = [];
  const proposerPdas: Address[] = [];
  for (const option of [0, 1]) {
    const { authority, proposer } = await proposeRealWithAuthority(f, oracle, option, BOND);
    authorities.push(authority);
    proposerPdas.push(proposer);
  }

  await advancePastPhaseEnd(f, oracle);
  await sendIx(f, await finalizeProposals({ oracle, proposers: proposerPdas }));

  const contentHash = new Uint8Array(32).fill(0x07);
  const submitter = await Keypair.generate();
  await f.harness.airdrop(submitter.publicKey.toString(), 2_000_000_000);
  const submitterKass = await fundKass(f, submitter.publicKey, 1_000_000n);
  await sendIx(
    f,
    await submitFact({ oracle, submitter: submitter.publicKey, submitterKass, contentHash, stake: 100n, uri: "ipfs://fact" }),
    [submitter],
  );
  const fact = (await pda.fact(oracle, contentHash)).address;

  await advancePastPhaseEnd(f, oracle);
  await sendIx(f, await advancePhase({ oracle }));

  const voter = await Keypair.generate();
  await f.harness.airdrop(voter.publicKey.toString(), 2_000_000_000);
  const voterKass = await fundKass(f, voter.publicKey, 10n * BOND);
  await sendIx(
    f,
    await voteFact({ oracle, fact, voter: voter.publicKey, voterKass, kind: VOTE_APPROVE, stake: 2n * BOND }),
    [voter],
  );

  await advancePastPhaseEnd(f, oracle);
  await sendIx(f, await finalizeFacts({ nonce, kassMint: f.kassMint.publicKey, tail: [fact] }));

  // --- submit_ai_claim via the APP builder (each proposer authority signs) ---
  for (let i = 0; i < proposerPdas.length; i++) {
    const ixs = await buildSubmitAiClaimIxs({
      oracle,
      proposer: proposerPdas[i],
      submitter: authorities[i].publicKey,
      modelId: new Uint8Array(32).fill(0xa1),
      paramsHash: new Uint8Array(32).fill(0xb2),
      ioHash: new Uint8Array(32).fill(0xc3),
      option: aiOption,
      optionsCount: 2,
    });
    await sendViaApp(f, authorities[i], ixs);
  }

  await advancePastPhaseEnd(f, oracle);
  await sendIx(f, await finalizeAiClaims({ oracle, proposers: proposerPdas }));

  const proposer = proposerPdas[0];
  const aiClaim = (await pda.aiClaim(oracle, proposer)).address;
  return { oracle, proposer, proposerAuthority: authorities[0].publicKey, aiClaim, proposerPdas, authorities };
}

export interface MarketComposition {
  question: Address;
  kass: VaultAccounts;
  usdc: VaultAccounts;
  oraclePassKass: Address;
  oracleFailKass: Address;
}

export async function composeMarket(f: Fixture, oracle: Address): Promise<MarketComposition> {
  const questionId = new Uint8Array(32).fill(0x07);
  const { question } = await composeQuestion(f, oracle, questionId, 2);
  const kass = await composeVault(f, question, f.kassMint.publicKey);
  const usdc = await composeVault(f, question, f.usdcMint.publicKey);
  const oraclePassKass = await fabricateTokenAccountMint(f, kass.passMint, oracle, 0n);
  const oracleFailKass = await fabricateTokenAccountMint(f, kass.failMint, oracle, 0n);
  return { question, kass, usdc, oraclePassKass, oracleFailKass };
}

/** openChallenge via `buildOpenChallengeIxs` → the app seam (challenger is fee-payer + signer). */
export async function openChallengeViaApp(
  f: Fixture,
  nonce: bigint,
  c: Challenged,
  m: MarketComposition,
  passAmm: Address,
  failAmm: Address,
): Promise<Keypair> {
  const challenger = await Keypair.generate();
  await f.harness.airdrop(challenger.publicKey.toString(), 2_000_000_000);
  const challengerUsdcSrc = await fabricateTokenAccountMint(f, f.usdcMint.publicKey, challenger.publicKey, 5_000_000n);
  const cvEventAuthority = (await Address.findProgramAddress([enc.encode("__event_authority")], VLTX))[0];

  const ixs = await buildOpenChallengeIxs({
    oracleNonce: nonce,
    proposer: c.proposer,
    challenger: challenger.publicKey,
    question: m.question,
    kassVault: m.kass.vault,
    usdcVault: m.usdc.vault,
    passAmm,
    failAmm,
    kassVaultUnderlying: m.kass.underlying,
    passKassMint: m.kass.passMint,
    failKassMint: m.kass.failMint,
    oraclePassKass: m.oraclePassKass,
    oracleFailKass: m.oracleFailKass,
    cvEventAuthority,
    kassDao: f.kassDao,
    usdcMint: f.usdcMint.publicKey,
    challengerUsdcSrc,
  });
  await sendViaApp(f, challenger, ixs, 1_400_000);
  return challenger;
}

export interface Payouts {
  escrowVault: Address;
  proposerUsdc: Address;
  challengerUsdcDest: Address;
  challengerKass: Address;
}

/**
 * settleChallenge via the SD1 `buildSettleFromMarketIxs` — DERIVING the full 15
 * settle accounts from the DECODED on-chain {@link decodeMarket} + Oracle (NOT
 * the composed-account JSON `m`), then sending through the app seam
 * (permissionless; payer sends). The three payout destinations are the DERIVED
 * ATAs (proposer-authority USDC, challenger USDC + KASS); we fabricate token
 * accounts AT those ATA addresses so the handler's `assert_token_account` +
 * transfers land on them, exactly as they would on a real cluster after an
 * idempotent create. Asserts each derived account == what the market was composed
 * with.
 */
export async function settleChallengeViaApp(
  f: Fixture,
  nonce: bigint,
  c: Challenged,
  m: MarketComposition,
  market: Address,
  challenger: Keypair,
  passAmm: Address,
  failAmm: Address,
): Promise<Payouts> {
  // DECODE the on-chain Market + Oracle — the derive source (no composed JSON).
  const decodedMarket = decodeMarket(await fetchAccount(f, market));
  const decodedOracle = decodeOracle(await fetchAccount(f, c.oracle));

  // The derived payout ATAs (owner: proposer.authority / challenger).
  const proposerUsdc = await ata(c.proposerAuthority, f.usdcMint.publicKey);
  const challengerUsdcDest = await ata(challenger.publicKey, f.usdcMint.publicKey);
  const challengerKass = await ata(challenger.publicKey, f.kassMint.publicKey);
  await setTokenAccountAt(f, proposerUsdc, f.usdcMint.publicKey, c.proposerAuthority, 0n);
  await setTokenAccountAt(f, challengerUsdcDest, f.usdcMint.publicKey, challenger.publicKey, 0n);
  await setTokenAccountAt(f, challengerKass, f.kassMint.publicKey, challenger.publicKey, 0n);
  const escrowVault = (await pda.challengeUsdcVault(market)).address;

  const twapEnd = decodedMarket.twapEnd;
  await f.harness.advanceToUnix(twapEnd + 120n);

  const ixs = await buildSettleFromMarketIxs({
    oracleNonce: nonce,
    market: decodedMarket,
    oracle: decodedOracle,
    proposerAuthority: c.proposerAuthority,
  });

  // The derived settle ix binds EXACTLY the accounts the market was composed with
  // (proves the derivation is correct against the real Market, pre-flight).
  const settleIx = ixs[ixs.length - 1];
  const keys = settleIx.keys.map((k) => k.pubkey.toString());
  expect(keys[2]).toBe(c.aiClaim.toString());
  expect(keys[3]).toBe(c.proposer.toString());
  expect(keys[4]).toBe(m.question.toString());
  expect(keys[5]).toBe(passAmm.toString());
  expect(keys[6]).toBe(failAmm.toString());
  expect(keys[11]).toBe(m.kass.vault.toString());
  expect(keys[12]).toBe(m.kass.underlying.toString());
  expect(keys[13]).toBe(m.kass.passMint.toString());
  expect(keys[14]).toBe(m.kass.failMint.toString());
  expect(keys[15]).toBe(m.oraclePassKass.toString());
  expect(keys[16]).toBe(m.oracleFailKass.toString());
  expect(keys[18]).toBe(proposerUsdc.toString());
  expect(keys[19]).toBe(challengerUsdcDest.toString());
  expect(keys[20]).toBe(challengerKass.toString());

  await sendViaApp(f, f.payer, ixs, 1_400_000);
  return { escrowVault, proposerUsdc, challengerUsdcDest, challengerKass };
}
