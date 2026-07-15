/**
 * Core seeding primitives for the browser E2E: the {@link SeedCtx}, boot/init,
 * tx sending, account fetch/fund, oracle creation, and the account-fabrication
 * cheatcodes (governance / kass_dao / window patching). Pure move/extract from
 * the former monolithic `seed.ts` — see `seed.ts` (barrel) + `seed-drivers.ts`.
 *
 * IMPORTANT: every pubkey handed to an `@kassandra-market/oracles` builder is passed as a
 * base58 STRING (`.toString()`), never a web3.js `Address` object — under
 * Playwright's loader the app and the SDK resolve separate copies of
 * `@solana/web3.js`, so a foreign `Address` fails the SDK's `instanceof` check.
 */
import { Address, Keypair, Transaction, type TransactionInstruction } from '@solana/web3.js'
import { buildDaoBlob } from '../../sdks/oracles/ts/test/surfpool/futarchy-dao.ts'
import {
  TOKEN_PROGRAM_ID,
  createOracle,
  pda,
  writeOracleMeta,
} from '@kassandra-market/oracles'

import { buildOracleMetadataJson } from '../src/data/actions/create.ts'
import { SurfpoolHarness, mintBytes, toHex, tokenAccountBytes } from '../../sdks/oracles/ts/test/surfpool/harness.ts'
import { MockAnthropic } from '../../sdks/oracles/ts/test/surfpool/mock-anthropic.ts'
import {
  runRunner,
  runnerAvailable,
  writeRunnerConfig,
  type RunOutput,
} from '../../sdks/oracles/ts/test/surfpool/run-runner.ts'

export interface SeedCtx {
  harness: SurfpoolHarness
  payer: Keypair
  kassMint: Keypair
  usdcMint: Keypair
}

async function sha256(s: string): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(s)))
}

/** 64-char hex → the 32-byte array the SDK builders expect. */
function hex32(h: string): Uint8Array {
  const b = new Uint8Array(32)
  for (let i = 0; i < 32; i++) b[i] = parseInt(h.slice(i * 2, i * 2 + 2), 16)
  return b
}

/** The AI-claim hashes the app's "Paste runner output" form accepts. */
export interface RunnerClaim {
  modelId: Uint8Array
  paramsHash: Uint8Array
  ioHash: Uint8Array
  option: number
  /** JSON the app's SubmitAiClaimForm paste-mode consumes verbatim. */
  formPayload: { model_id: string; params_hash: string; io_hash: string; option: number }
}

/**
 * Run the REAL `kassandra-runner` binary — its genuine AnthropicProvider
 * HTTP+parse path — against a LOCAL MOCK Anthropic server (no API key, no
 * network) to produce a real AI-claim payload for `option`. The e2e uses THIS
 * instead of fabricated hashes so the runner is actually exercised end to end.
 *
 * Uses zero facts (the runner accepts an empty fact set), so nothing is fetched
 * over the network; the mock supplies the model's verdict.
 */
export async function runnerClaim(option: number, optionsCount = 2): Promise<RunnerClaim> {
  if (!runnerAvailable()) {
    throw new Error(
      'kassandra-runner binary missing — build it first: `cargo build -p kassandra-runner`',
    )
  }
  const mock = await MockAnthropic.start()
  try {
    mock.setOption(option, 'claude-opus-4-8')
    const configPath = writeRunnerConfig({
      interpretation: 'E2E: resolve the disputed oracle to the AI-selected option.',
      options_count: optionsCount,
      option_labels: Array.from({ length: optionsCount }, (_, i) => ({
        index: i,
        label: `Option ${i}`,
      })),
      facts: [],
    })
    const { code, stdout, stderr } = await runRunner(configPath, mock.baseUrl)
    if (code !== 0) throw new Error(`kassandra-runner exited ${code}: ${stderr}`)
    const out = JSON.parse(stdout) as RunOutput
    return {
      modelId: hex32(out.model_id_hex),
      paramsHash: hex32(out.params_hash_hex),
      ioHash: hex32(out.io_hash_hex),
      option: out.option_index,
      formPayload: {
        model_id: out.model_id_hex,
        params_hash: out.params_hash_hex,
        io_hash: out.io_hash_hex,
        option: out.option_index,
      },
    }
  } finally {
    await mock.stop()
  }
}

/** Boot surfpool, deploy the program, mint KASS/USDC, and init the protocol. */
export async function bootAndInit(
  port: number,
  harnessOpts: Record<string, unknown> = {},
): Promise<SeedCtx> {
  const harness = await SurfpoolHarness.start({ port, ...harnessOpts })
  const payer = await Keypair.generate()
  await harness.airdrop(payer.publicKey.toString(), 1_000_000_000_000)

  const { mintAuthority, initProtocol } = await import('@kassandra-market/oracles')
  const mintAuth = await mintAuthority()
  const kassMint = await Keypair.generate()
  const usdcMint = await Keypair.generate()
  await harness.setAccount(kassMint.publicKey.toString(), {
    lamports: 1_000_000_000,
    owner: TOKEN_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(mintBytes(mintAuth.address.toBytes(), 10n ** 18n, 9)),
  })
  await harness.setAccount(usdcMint.publicKey.toString(), {
    lamports: 1_000_000_000,
    owner: TOKEN_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(mintBytes(payer.publicKey.toBytes(), 0n, 6)),
  })

  const ctx: SeedCtx = { harness, payer, kassMint, usdcMint }
  await sendIx(
    ctx,
    await initProtocol({
      admin: payer.publicKey.toString(),
      kassMint: kassMint.publicKey.toString(),
      usdcMint: usdcMint.publicKey.toString(),
    }),
  )
  return ctx
}

/** Send one ix signed by the payer (+ extra signers). */
export async function sendIx(
  ctx: SeedCtx,
  ix: TransactionInstruction,
  signers: Keypair[] = [],
): Promise<void> {
  await sendIxs(ctx, [ix], signers)
}

/** Send several ixs in ONE tx signed by the payer (+ extra signers). */
export async function sendIxs(
  ctx: SeedCtx,
  ixs: TransactionInstruction[],
  signers: Keypair[] = [],
): Promise<void> {
  const conn = ctx.harness.connection
  const tx = new Transaction()
  tx.feePayer = ctx.payer.publicKey
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash
  for (const ix of ixs) tx.add(ix)
  await tx.sign(ctx.payer, ...signers)
  const sig = await conn.sendRawTransaction(await tx.serialize(), { skipPreflight: false })
  await ctx.harness.confirmSignature(sig)
}

export async function fetchAccount(ctx: SeedCtx, address: Address): Promise<Uint8Array> {
  const deadline = Date.now() + 15_000
  while (Date.now() < deadline) {
    const info = await ctx.harness.connection.getAccountInfo(address)
    if (info && info.data.length > 0) return info.data
    await new Promise((r) => setTimeout(r, 150))
  }
  throw new Error(`account ${address} did not appear`)
}

/** Fabricate a KASS token account owned by `owner` (base58) holding `amount`. */
export async function fundKass(ctx: SeedCtx, owner: string, amount: bigint): Promise<string> {
  const ownerBytes = new Address(owner).toBytes()
  const acct = await Keypair.generate()
  await ctx.harness.setAccount(acct.publicKey.toString(), {
    lamports: 5_000_000,
    owner: TOKEN_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(tokenAccountBytes(ctx.kassMint.publicKey.toBytes(), ownerBytes, amount)),
  })
  return acct.publicKey.toString()
}

/** Create an oracle with `optionsCount` options, creator = payer, deadline in the near future. */
export async function createOracleReal(
  ctx: SeedCtx,
  nonce: bigint,
  optionsCount: number,
  question: string,
): Promise<Address> {
  const creatorKass = await fundKass(ctx, ctx.payer.publicKey.toString(), 10n ** 15n)
  const now = await ctx.harness.clockUnixTimestamp()
  const createIx = await createOracle({
    nonce,
    optionsCount,
    deadline: now + 1_000n + nonce * 100n,
    twapWindow: 600n,
    creator: ctx.payer.publicKey.toString(),
    creatorKassToken: creatorKass,
    kassMint: ctx.kassMint.publicKey.toString(),
    usdcMint: ctx.usdcMint.publicKey.toString(),
  })
  // Write the on-chain metadata (subject + labels + uri/uri_hash) in the SAME tx,
  // so the indexer mirrors it and the browse/detail views show it. `uri` points at
  // the app's metadata host when APP_ORIGIN is set (best-effort POST below).
  const oracle = (await pda.oracle(nonce)).address
  const options = Array.from({ length: optionsCount }, (_, i) => `Option ${i}`)
  const json = buildOracleMetadataJson({ subject: question, options })
  const jsonString = JSON.stringify(json)
  const uriHash = await sha256(jsonString)
  const appOrigin = process.env.APP_ORIGIN?.replace(/\/$/, '') ?? ''
  const uri = appOrigin ? `${appOrigin}/api/oracle/${oracle.toString()}/metadata.json` : ''
  const metaIx = await writeOracleMeta({
    oracle,
    creator: ctx.payer.publicKey.toString(),
    subject: question,
    options,
    uri,
    uriHash,
  })
  await sendIxs(ctx, [createIx, metaIx])
  if (appOrigin) {
    // Best-effort: host the extended JSON so the on-chain `uri` resolves.
    try {
      await fetch(uri, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: jsonString,
      })
    } catch {
      // Ignore — the on-chain subject/options still index + display.
    }
  }
  return oracle
}

/**
 * How far ahead of the current chain clock to push a kept-open window's
 * `phase_ends_at`. It must clear every clock advance for the rest of seeding AND
 * the ensuing browse/test session — but seeding only elapses a handful of ~1h
 * phase windows (a few hours of chain time in practice), so a WEEK is ample
 * headroom. It must NOT be absurdly large (the old value was 1e9 s ≈ 31 years),
 * because the app renders the remaining time literally ("ends in …"): a 1e9-s
 * window shows as "ends in 11574d", which reads as broken. A week shows a sane
 * "ends in 7d".
 */
const KEEP_OPEN_AHEAD_SECS = 7n * 24n * 3600n

/**
 * Push an oracle's `phase_ends_at` (i64 at byte offset 144) into the near future
 * (see {@link KEEP_OPEN_AHEAD_SECS}) so its CURRENT phase window stays OPEN at
 * test/browse time — regardless of how far later seeding advances the shared
 * surfpool clock. surfpool's time-travel is forward-only, so we cannot rewind
 * the clock into a closed window; instead we move the window's end past every
 * clock position seeding will reach. The phase itself is unchanged, so the
 * phase-gated action (submit fact / vote / submit AI claim) is still legal.
 */
export async function keepWindowOpen(ctx: SeedCtx, oracle: Address): Promise<void> {
  const { KASSANDRA_PROGRAM_ID } = await import('@kassandra-market/oracles')
  const info = await ctx.harness.connection.getAccountInfo(oracle)
  if (!info) throw new Error(`oracle ${oracle} not found for window patch`)
  const data = Uint8Array.from(info.data as Uint8Array)
  const now = await ctx.harness.clockUnixTimestamp()
  new DataView(data.buffer).setBigInt64(144, now + KEEP_OPEN_AHEAD_SECS, true)
  await ctx.harness.setAccount(oracle.toString(), {
    lamports: Number((info as { lamports?: bigint | number }).lamports ?? 5_000_000),
    owner: KASSANDRA_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(data),
  })
}

/**
 * Fabricate DAO governance: patch the Protocol singleton so `governance_set = 1`
 * and `dao_authority = daoAuthority` (offsets 121 / 128), and create the DAO
 * treasury (`ATA(daoAuthority, kass_mint)`) as an empty KASS token account. The
 * real set_governance is hardened (dao_authority must equal a Squads vault PDA no
 * keypair can sign), so tests fabricate the linkage directly — exactly as the
 * gated `claims.e2e` surfpool test documents.
 */
export async function fabricateGovernance(ctx: SeedCtx, daoAuthority: string): Promise<void> {
  const { KASSANDRA_PROGRAM_ID, associatedTokenAccount } = await import('@kassandra-market/oracles')
  const p = (await pda.protocol()).address
  const info = await ctx.harness.connection.getAccountInfo(p)
  if (!info) throw new Error('protocol not found')
  const data = Uint8Array.from(info.data as Uint8Array)
  data[121] = 1 // governance_set
  data.set(new Address(daoAuthority).toBytes(), 128) // dao_authority
  await ctx.harness.setAccount(p.toString(), {
    lamports: Number((info as { lamports?: bigint | number }).lamports ?? 5_000_000),
    owner: KASSANDRA_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(data),
  })
  // DAO treasury = ATA(dao_authority, kass_mint), empty.
  const treasury = (await associatedTokenAccount(daoAuthority, ctx.kassMint.publicKey.toString()))
    .address
  await ctx.harness.setAccount(treasury.toString(), {
    lamports: 5_000_000,
    owner: TOKEN_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(
      tokenAccountBytes(ctx.kassMint.publicKey.toBytes(), new Address(daoAuthority).toBytes(), 0n),
    ),
  })
}

/**
 * Back-date an oracle's `phase_ends_at` (offset 144) to ~40 days in the PAST
 * (real time), so the sweep's 30-day grace is elapsed for BOTH the browser gate
 * (SweepControl compares `Date.now()` against `phase_ends_at + grace`) and the
 * program gate (the surfpool clock is well past a real-time-past timestamp).
 */
export async function backdateForSweep(ctx: SeedCtx, oracle: Address): Promise<void> {
  const { KASSANDRA_PROGRAM_ID } = await import('@kassandra-market/oracles')
  const info = await ctx.harness.connection.getAccountInfo(oracle)
  if (!info) throw new Error('oracle not found for backdate')
  const data = Uint8Array.from(info.data as Uint8Array)
  const past = BigInt(Math.floor(Date.now() / 1000) - 40 * 24 * 3600)
  new DataView(data.buffer).setBigInt64(144, past, true)
  await ctx.harness.setAccount(oracle.toString(), {
    lamports: Number((info as { lamports?: bigint | number }).lamports ?? 5_000_000),
    owner: KASSANDRA_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(data),
  })
}
/** Patch the Protocol singleton bytes in place (for governance fabrication). */
async function patchProtocolBytes(ctx: SeedCtx, mutate: (d: Uint8Array) => void): Promise<void> {
  const { KASSANDRA_PROGRAM_ID } = await import('@kassandra-market/oracles')
  const p = (await pda.protocol()).address
  const info = await ctx.harness.connection.getAccountInfo(p)
  if (!info) throw new Error('protocol not found')
  const data = Uint8Array.from(info.data as Uint8Array)
  mutate(data)
  await ctx.harness.setAccount(p.toString(), {
    lamports: Number((info as { lamports?: bigint | number }).lamports ?? 5_000_000),
    owner: KASSANDRA_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(data),
  })
}

/**
 * Fabricate a futarchy-owned `Dao` account carrying a spot TWAP and record it as
 * `Protocol.kass_dao` (offset 160) — the account `kass_price` reads and the
 * linkage `set_governance` needs. Returns the DAO address.
 */
export async function fabricateKassDao(ctx: SeedCtx): Promise<string> {
  const { EXTERNAL_PROGRAM_IDS } = await import('@kassandra-market/oracles')
  const dao = await Keypair.generate()
  await ctx.harness.setAccount(dao.publicKey.toString(), {
    lamports: 5_000_000,
    owner: EXTERNAL_PROGRAM_IDS.futarchyV06.toString(),
    executable: false,
    data: toHex(buildDaoBlob(500_000_000n * 1_000_000n, 1_000_000n, 0n)),
  })
  await patchProtocolBytes(ctx, (d) => d.set(dao.publicKey.toBytes(), 160))
  return dao.publicKey.toString()
}
