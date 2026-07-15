/**
 * Shared offline fixtures/builders for the CU3 challenge-market compose tests
 * (`challengeCompose.unit.test.ts`). Pure move/extract — no behavior change.
 */
import { Keypair, TransactionInstruction, type Connection } from "@solana/web3.js";
import { EXTERNAL_PROGRAM_IDS } from "@kassandra-market/oracles";
import { expect } from "vitest";

import { buildComposeAndOpenChallengeIxs } from "../../src/data/actions/challengeCompose.ts";

export const enc = new TextEncoder();
export const VLTX = EXTERNAL_PROGRAM_IDS.conditionalVault;

/** A connection whose ATA-existence check always reports absent (create fires). */
export function fakeConnection(): Connection {
  return { getAccountInfo: async () => null } as unknown as Connection;
}

export function keyShape(ix: TransactionInstruction) {
  return ix.keys.map((k) => ({
    pubkey: k.pubkey.toString(),
    isSigner: k.isSigner,
    isWritable: k.isWritable,
  }));
}

export function expectIxMatches(actual: TransactionInstruction, expected: TransactionInstruction) {
  expect(actual.programId.toString()).toBe(expected.programId.toString());
  expect(Array.from(actual.data)).toEqual(Array.from(expected.data));
  expect(keyShape(actual)).toEqual(keyShape(expected));
}

/** A deterministic fixture: the nonce, proposer, challenger, mints, dao. */
export async function fixture() {
  const nonce = 100n;
  const proposer = (await Keypair.generate()).publicKey;
  const challenger = (await Keypair.generate()).publicKey;
  const kassMint = (await Keypair.generate()).publicKey;
  const usdcMint = (await Keypair.generate()).publicKey;
  const kassDao = (await Keypair.generate()).publicKey;
  return { nonce, proposer, challenger, kassMint, usdcMint, kassDao };
}

export async function build(over: Partial<Parameters<typeof buildComposeAndOpenChallengeIxs>[0]> = {}) {
  const f = await fixture();
  return buildComposeAndOpenChallengeIxs({
    connection: fakeConnection(),
    oracleNonce: f.nonce,
    proposer: f.proposer,
    challenger: f.challenger,
    kassMint: f.kassMint,
    usdcMint: f.usdcMint,
    kassDao: f.kassDao,
    ...over,
  });
}
