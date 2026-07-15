/**
 * Phase drivers for the browser E2E: push an oracle through propose → fact →
 * vote → AI-claim → challenge → resolve with REAL instructions, plus the
 * specific-wallet variants the specs need. Pure move/extract from the former
 * monolithic `seed.ts` — consumes the primitives in `seed-core.ts`.
 *
 * IMPORTANT: every pubkey handed to an `@kassandra-market/oracles` builder is passed as a
 * base58 STRING (`.toString()`), never a web3.js `Address` object.
 */
import { Address, Keypair } from '@solana/web3.js'
import {
  VOTE_APPROVE,
  advancePhase,
  decodeOracle,
  finalizeAiClaims,
  finalizeFacts,
  finalizeProposals,
  pda,
  propose,
  submitAiClaim,
  submitFact,
  voteFact,
} from '@kassandra-market/oracles'

import { toHex } from '../../sdks/oracles/ts/test/surfpool/harness.ts'
import {
  type RunnerClaim,
  type SeedCtx,
  createOracleReal,
  fetchAccount,
  fundKass,
  runnerClaim,
  sendIx,
} from './seed-core.ts'

/** Seed an oracle stuck in InvalidDeadend (phaseRaw byte 161 = 8) for resolve_deadend. */
export async function seedDeadendOracle(ctx: SeedCtx, nonce: bigint): Promise<Address> {
  const { KASSANDRA_PROGRAM_ID } = await import('@kassandra-market/oracles')
  const o = await createOracleReal(ctx, nonce, 2, 'E2E dead-end')
  await driveToResolvedUncontested(ctx, o, 0)
  const info = await ctx.harness.connection.getAccountInfo(o)
  if (!info) throw new Error('deadend oracle not found')
  const data = Uint8Array.from(info.data as Uint8Array)
  data[161] = 8 // Phase::InvalidDeadend
  await ctx.harness.setAccount(o.toString(), {
    lamports: Number((info as { lamports?: bigint | number }).lamports ?? 5_000_000),
    owner: KASSANDRA_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(data),
  })
  return o
}

/** Resolve an oracle uncontested (all proposers agree) → Resolved(option). */
export async function driveToResolvedUncontested(
  ctx: SeedCtx,
  oracle: Address,
  option: number,
): Promise<void> {
  await openProposals(ctx, oracle)
  const p: string[] = []
  for (let i = 0; i < 3; i++) {
    p.push((await proposeAs(ctx, oracle, await Keypair.generate(), option, 5_000n)).toString())
  }
  await advancePastPhaseEnd(ctx, oracle)
  await sendIx(ctx, await finalizeProposals({ oracle: oracle.toString(), proposers: p }))
}

export async function openProposals(ctx: SeedCtx, oracle: Address): Promise<void> {
  const o = decodeOracle(await fetchAccount(ctx, oracle))
  await ctx.harness.advanceToUnix(o.deadline + 60n)
}

export async function advancePastPhaseEnd(ctx: SeedCtx, oracle: Address): Promise<void> {
  const o = decodeOracle(await fetchAccount(ctx, oracle))
  await ctx.harness.advanceToUnix(o.phaseEndsAt + 120n)
}

/**
 * Propose `option` with `bond` from `authority` (a caller-supplied keypair —
 * pass the funded browser wallet to make it a locked-in proposer). Funds the
 * authority's KASS bond source.
 */
export async function proposeAs(
  ctx: SeedCtx,
  oracle: Address,
  authority: Keypair,
  option: number,
  bond: bigint,
): Promise<Address> {
  await ctx.harness.airdrop(authority.publicKey.toString(), 2_000_000_000)
  const authorityKass = await fundKass(ctx, authority.publicKey.toString(), bond * 10n)
  await sendIx(
    ctx,
    await propose({
      oracle: oracle.toString(),
      authority: authority.publicKey.toString(),
      authorityKass,
      option,
      bond,
    }),
    [authority],
  )
  return (await pda.proposer(oracle.toString(), authority.publicKey.toString())).address
}

/** Drive an oracle into FactProposal via a 2-proposer dispute. Returns the proposer PDAs. */
export async function driveToFactProposal(
  ctx: SeedCtx,
  oracle: Address,
  walletProposer?: Keypair,
): Promise<Address[]> {
  await openProposals(ctx, oracle)
  const proposers: Address[] = []
  const a0 = walletProposer ?? (await Keypair.generate())
  proposers.push(await proposeAs(ctx, oracle, a0, 0, 1_000n))
  const a1 = await Keypair.generate()
  proposers.push(await proposeAs(ctx, oracle, a1, 1, 1_000n))
  await advancePastPhaseEnd(ctx, oracle)
  await sendIx(ctx, await finalizeProposals({ oracle: oracle.toString(), proposers: proposers.map(String) }))
  return proposers
}

/** Submit one fact (from a fresh submitter) into a FactProposal oracle. Returns the Fact PDA. */
export async function submitOneFact(ctx: SeedCtx, oracle: Address): Promise<Address> {
  const contentHash = new Uint8Array(32).fill(0x07)
  const submitter = await Keypair.generate()
  await ctx.harness.airdrop(submitter.publicKey.toString(), 2_000_000_000)
  const submitterKass = await fundKass(ctx, submitter.publicKey.toString(), 1_000_000n)
  await sendIx(
    ctx,
    await submitFact({
      oracle: oracle.toString(),
      submitter: submitter.publicKey.toString(),
      submitterKass,
      contentHash,
      stake: 100n,
      uri: 'ipfs://seeded-fact',
    }),
    [submitter],
  )
  return (await pda.fact(oracle.toString(), contentHash)).address
}

/** Advance FactProposal → FactVoting. */
export async function advanceToFactVoting(ctx: SeedCtx, oracle: Address): Promise<void> {
  await advancePastPhaseEnd(ctx, oracle)
  await sendIx(ctx, await advancePhase({ oracle: oracle.toString() }))
}

/** Approve-vote a fact (fresh voter, clears quorum). */
export async function approveVote(ctx: SeedCtx, oracle: Address, fact: Address): Promise<void> {
  const voter = await Keypair.generate()
  await ctx.harness.airdrop(voter.publicKey.toString(), 2_000_000_000)
  const voterKass = await fundKass(ctx, voter.publicKey.toString(), 10_000n)
  await sendIx(
    ctx,
    await voteFact({
      oracle: oracle.toString(),
      fact: fact.toString(),
      voter: voter.publicKey.toString(),
      voterKass,
      kind: VOTE_APPROVE,
      stake: 2_000n,
    }),
    [voter],
  )
}

/** FactVoting → AiClaim (finalize the fact set). */
export async function advanceToAiClaim(ctx: SeedCtx, oracle: Address, nonce: bigint, fact: Address): Promise<void> {
  await advancePastPhaseEnd(ctx, oracle)
  await sendIx(
    ctx,
    await finalizeFacts({ nonce, kassMint: ctx.kassMint.publicKey.toString(), tail: [fact.toString()] }),
  )
}

/** AiClaim → Challenge (finalize the AI-claim round over the proposer tail). */
export async function advanceToChallenge(ctx: SeedCtx, oracle: Address, proposers: Address[]): Promise<void> {
  await advancePastPhaseEnd(ctx, oracle)
  await sendIx(ctx, await finalizeAiClaims({ oracle: oracle.toString(), proposers: proposers.map(String) }))
}

/**
 * Drive an oracle all the way to the Challenge phase with `wallet` surviving:
 * dispute (wallet = proposer 0) → a fact → FactVoting → approve → AiClaim →
 * the wallet stamps its AI claim (so it is NOT slashed) → finalize_ai_claims →
 * Challenge (open_challenge_count == 0). Returns the wallet's Proposer PDA + the
 * fact + the wallet's AiClaim PDA.
 */
export async function driveToChallengeSurviving(
  ctx: SeedCtx,
  oracle: Address,
  nonce: bigint,
  wallet: Keypair,
): Promise<{ proposers: Address[]; fact: Address; aiClaim: Address }> {
  const proposers = await driveToFactProposal(ctx, oracle, wallet)
  const fact = await submitOneFact(ctx, oracle)
  await advanceToFactVoting(ctx, oracle)
  await approveVote(ctx, oracle, fact)
  await advanceToAiClaim(ctx, oracle, nonce, fact)
  await submitAiClaimAs(ctx, oracle, proposers[0], wallet, 0)
  await advanceToChallenge(ctx, oracle, proposers)
  const aiClaim = (await pda.aiClaim(oracle.toString(), proposers[0].toString())).address
  return { proposers, fact, aiClaim }
}

/** Finalize a Challenge-phase oracle (no open challenges) → terminal (Resolved). */
export async function finalizeToTerminal(
  ctx: SeedCtx,
  oracle: Address,
  nonce: bigint,
  proposers: Address[],
): Promise<void> {
  const { finalizeOracle } = await import('@kassandra-market/oracles')
  await advancePastPhaseEnd(ctx, oracle)
  await sendIx(
    ctx,
    await finalizeOracle({
      nonce,
      kassMint: ctx.kassMint.publicKey.toString(),
      proposers: proposers.map(String),
    }),
  )
}

/** Submit a fact as a SPECIFIC keypair (e.g. the browser wallet). Returns the Fact PDA. */
export async function submitFactAs(
  ctx: SeedCtx,
  oracle: Address,
  submitter: Keypair,
  stake: bigint,
): Promise<Address> {
  const contentHash = new Uint8Array(32).fill(0x5a)
  await ctx.harness.airdrop(submitter.publicKey.toString(), 2_000_000_000)
  const submitterKass = await fundKass(ctx, submitter.publicKey.toString(), stake * 10n)
  await sendIx(
    ctx,
    await submitFact({
      oracle: oracle.toString(),
      submitter: submitter.publicKey.toString(),
      submitterKass,
      contentHash,
      stake,
      uri: 'ipfs://wallet-fact',
    }),
    [submitter],
  )
  return (await pda.fact(oracle.toString(), contentHash)).address
}

/** Approve-vote a fact as a SPECIFIC keypair (e.g. the browser wallet). */
export async function voteFactAs(
  ctx: SeedCtx,
  oracle: Address,
  fact: Address,
  voter: Keypair,
  stake: bigint,
): Promise<void> {
  await ctx.harness.airdrop(voter.publicKey.toString(), 2_000_000_000)
  const voterKass = await fundKass(ctx, voter.publicKey.toString(), stake * 10n)
  await sendIx(
    ctx,
    await voteFact({
      oracle: oracle.toString(),
      fact: fact.toString(),
      voter: voter.publicKey.toString(),
      voterKass,
      kind: VOTE_APPROVE,
      stake,
    }),
    [voter],
  )
}

/**
 * Drive an oracle all the way to Resolved with `wallet` in EVERY claimable role:
 * winning proposer (option 0), agreed-fact submitter, approve-voter, and AI
 * claimant. The second proposer (option 1) submits no AI claim, so it is slashed
 * and the wallet's option resolves. Returns the wallet's claimable child PDAs.
 */
export async function driveToResolvedFull(
  ctx: SeedCtx,
  oracle: Address,
  nonce: bigint,
  wallet: Keypair,
): Promise<{ proposer: Address; fact: Address; factVote: Address; aiClaim: Address }> {
  await openProposals(ctx, oracle)
  const walletProposer = await proposeAs(ctx, oracle, wallet, 0, 5_000n)
  const other = await proposeAs(ctx, oracle, await Keypair.generate(), 1, 1_000n)
  await advancePastPhaseEnd(ctx, oracle)
  await sendIx(ctx, await finalizeProposals({ oracle: oracle.toString(), proposers: [walletProposer.toString(), other.toString()] }))

  const fact = await submitFactAs(ctx, oracle, wallet, 2_000n)
  await advanceToFactVoting(ctx, oracle)
  await voteFactAs(ctx, oracle, fact, wallet, 8_000n) // clears quorum vs the 6000 bond weight
  await advanceToAiClaim(ctx, oracle, nonce, fact)
  await submitAiClaimAs(ctx, oracle, walletProposer, wallet, 0)
  await advanceToChallenge(ctx, oracle, [walletProposer, other])
  await finalizeToTerminal(ctx, oracle, nonce, [walletProposer, other])

  const factVote = (await pda.factVote(fact.toString(), wallet.publicKey.toString())).address
  const aiClaim = (await pda.aiClaim(oracle.toString(), walletProposer.toString())).address
  return { proposer: walletProposer, fact, factVote, aiClaim }
}

/**
 * Submit an AI claim as `authority` for its `proposer`, using hashes produced by
 * the REAL runner (mock Anthropic — see {@link runnerClaim}), not fabricated ones.
 * Returns the runner claim so callers can reuse its payload (e.g. the browser
 * paste-mode test).
 */
export async function submitAiClaimAs(
  ctx: SeedCtx,
  oracle: Address,
  proposer: Address,
  authority: Keypair,
  option: number,
): Promise<RunnerClaim> {
  const claim = await runnerClaim(option)
  await sendIx(
    ctx,
    await submitAiClaim({
      oracle: oracle.toString(),
      proposer: proposer.toString(),
      authority: authority.publicKey.toString(),
      modelId: claim.modelId,
      paramsHash: claim.paramsHash,
      ioHash: claim.ioHash,
      option: claim.option,
    }),
    [authority],
  )
  return claim
}
