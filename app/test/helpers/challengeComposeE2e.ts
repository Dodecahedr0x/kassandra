/**
 * Fixture + front-door drivers for the CU3 forked-mainnet compose→open E2E
 * (`challengeCompose.e2e.test.ts`). The market composition itself is the app
 * builder under test, so only the front-door drivers are ported here. Pure
 * move/extract from the test file — no behavior change.
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
  VOTE_APPROVE,
  advancePhase,
  createOracle,
  decodeOracle,
  finalizeAiClaims,
  finalizeFacts,
  finalizeProposals,
  propose,
  submitFact,
  voteFact,
} from "@kassandra-market/oracles";
import * as pda from "@kassandra-market/oracles";

import {
  SurfpoolHarness,
  toHex,
  tokenAccountAmount,
  tokenAccountBytes,
} from "../../../sdks/oracles/ts/test/surfpool/harness.ts";
import { buildSubmitAiClaimIxs } from "../../src/data/actions/challenge.ts";
import { keypairSender, sendAndConfirm } from "../../src/data/send.ts";

export const BOND = 1_000_000_000n;

export interface Fixture {
  harness: SurfpoolHarness;
  payer: Keypair;
  kassMint: Keypair;
  usdcMint: Keypair;
  kassDao: Address;
}

export interface Challenged {
  oracle: Address;
  proposer: Address;
  aiClaim: Address;
  proposerPdas: Address[];
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
    await sendAndConfirm(f.harness.connection, keypairSender(f.harness.connection, authorities[i]), ixs);
  }

  await advancePastPhaseEnd(f, oracle);
  await sendIx(f, await finalizeAiClaims({ oracle, proposers: proposerPdas }));

  const proposer = proposerPdas[0];
  const aiClaim = (await pda.aiClaim(oracle, proposer)).address;
  return { oracle, proposer, aiClaim, proposerPdas };
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

async function fabricateTokenAccountMint(
  f: Fixture,
  mint: Address,
  owner: Address,
  amount: bigint,
): Promise<Address> {
  const acct = await Keypair.generate();
  await setTokenAccountAt(f, acct.publicKey, mint, owner, amount);
  return acct.publicKey;
}

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

async function fundKass(f: Fixture, owner: Address, amount: bigint): Promise<Address> {
  return fabricateTokenAccountMint(f, f.kassMint.publicKey, owner, amount);
}

async function createOracleReal(f: Fixture, nonce: bigint, optionsCount: number): Promise<void> {
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

async function openProposals(f: Fixture, oracle: Address): Promise<void> {
  const o = decodeOracle(await fetchAccount(f, oracle));
  await f.harness.advanceToUnix(o.deadline + 60n);
}

async function advancePastPhaseEnd(f: Fixture, oracle: Address): Promise<void> {
  const o = decodeOracle(await fetchAccount(f, oracle));
  await f.harness.advanceToUnix(o.phaseEndsAt + 120n);
}

async function proposeRealWithAuthority(
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
