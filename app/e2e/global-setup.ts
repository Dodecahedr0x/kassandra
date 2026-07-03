/**
 * Playwright globalSetup for the browser E2E — spins up ALL the local nodes the
 * app needs and funds the wallet keypair the browser will sign with.
 *
 * Steps (mirrors the gated surfpool E2Es in `app/test/*.e2e.test.ts`):
 *   1. Boot a headless surfpool simnet on :8899 and deploy the built program.
 *   2. init_protocol with fresh KASS/USDC mints (KASS authority = mint-authority PDA).
 *   3. Generate the USER wallet keypair; airdrop SOL + fund its KASS ATA (the
 *      creation-fee burn source + bond source).
 *   4. Seed a couple of oracles in the Proposal phase so the app has real data.
 *   5. Write the funded keypair's secret + the RPC url + seeded nonces to
 *      `e2e/.wallet.json`, which the specs inject into the browser as
 *      `window.__E2E_WALLET_SECRET__` for the real-signing e2e wallet.
 *
 * Returns a teardown that stops surfpool after the suite. The app dev server is
 * started separately by Playwright's `webServer` config (pointed at :8899 with
 * VITE_E2E=1).
 */
import { writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { Keypair, Transaction, type TransactionInstruction } from '@solana/web3.js'
import {
  TOKEN_PROGRAM_ID,
  associatedTokenAccount,
  createOracle,
  initProtocol,
  mintAuthority,
  pda,
} from '@kassandra/sdk'

import {
  SurfpoolHarness,
  mintBytes,
  toHex,
  tokenAccountBytes,
} from '../../sdk/test/surfpool/harness.ts'

// NOTE: seeding calls the SDK builders DIRECTLY (with base58-string args), never
// the app's `build*Ixs` wrappers. Under Playwright's loader the app and the SDK
// resolve separate copies of `@solana/web3.js`, so passing a web3.js `Address`
// object from the app layer into the SDK fails its `instanceof Address` check;
// staying inside the SDK's copy (strings in, SDK objects out) sidesteps it.

async function sha256(s: string): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(s)))
}

const PORT = 8899
const WALLET_FILE = join(process.cwd(), 'e2e', '.wallet.json')

async function globalSetup(): Promise<() => Promise<void>> {
  const harness = await SurfpoolHarness.start({ port: PORT })
  const rpcUrl = `http://127.0.0.1:${PORT}`

  const payer = await Keypair.generate()
  await harness.airdrop(payer.publicKey.toString(), 1_000_000_000_000)

  // Mints: KASS authority MUST be the program's mint-authority PDA (emission mint);
  // USDC authority is the payer.
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

  // NOTE: hand the SDK plain base58 strings (`.toString()`), not the web3.js
  // `Address` objects. Under Playwright's loader the SDK resolves a separate
  // copy of `@solana/web3.js`, so a foreign `Address` object fails the SDK's
  // `instanceof Address` check; a base58 string is accepted copy-agnostically.
  await sendIx(
    harness,
    payer,
    await initProtocol({
      admin: payer.publicKey.toString(),
      kassMint: kassMint.publicKey.toString(),
      usdcMint: usdcMint.publicKey.toString(),
    }),
  )

  // The funded USER wallet the browser signs with: SOL for rent/fees + KASS at
  // its canonical ATA (the create-fee burn + proposal-bond source).
  const wallet = await Keypair.generate()
  await harness.airdrop(wallet.publicKey.toString(), 50_000_000_000)
  const walletKass = (
    await associatedTokenAccount(wallet.publicKey.toString(), kassMint.publicKey.toString())
  ).address
  await harness.setAccount(walletKass.toString(), {
    lamports: 5_000_000,
    owner: TOKEN_PROGRAM_ID.toString(),
    executable: false,
    data: toHex(
      tokenAccountBytes(kassMint.publicKey.toBytes(), wallet.publicKey.toBytes(), 10n ** 15n),
    ),
  })

  // Seed a couple of oracles (Proposal phase) so the dashboard has real data.
  const nowUnix = await harness.clockUnixTimestamp()
  const seeded: { nonce: string; address: string }[] = []
  for (let i = 0; i < 2; i++) {
    const nonce = BigInt(i + 1)
    const ix = await createOracle({
      nonce,
      promptHash: await sha256(`E2E seed oracle #${i + 1}: did the funded browser wallet work?`),
      optionsCount: 3,
      deadline: nowUnix + 3_600n,
      twapWindow: 600n,
      creator: wallet.publicKey.toString(),
      creatorKassToken: walletKass.toString(),
      kassMint: kassMint.publicKey.toString(),
      usdcMint: usdcMint.publicKey.toString(),
    })
    // The creator (wallet) signs: it pays rent + is the fee-burn authority.
    await sendIx(harness, wallet, ix)
    const oracleAddr = (await pda.oracle(nonce)).address
    seeded.push({ nonce: nonce.toString(), address: oracleAddr.toString() })
  }

  writeFileSync(
    WALLET_FILE,
    JSON.stringify(
      {
        secretKey: Array.from(wallet.secretKey as Uint8Array),
        publicKey: wallet.publicKey.toString(),
        rpcUrl,
        kassMint: kassMint.publicKey.toString(),
        usdcMint: usdcMint.publicKey.toString(),
        oracles: seeded,
      },
      null,
      2,
    ),
  )

  // eslint-disable-next-line no-console
  console.log(
    `[e2e] surfpool on ${rpcUrl}; funded wallet ${wallet.publicKey.toString()}; seeded ${seeded.length} oracles`,
  )

  return async () => {
    await harness.teardown()
  }
}

/** Send a single instruction signed by `payer` (+ extra signers). */
async function sendIx(
  harness: SurfpoolHarness,
  payer: Keypair,
  ix: TransactionInstruction,
  signers: Keypair[] = [],
): Promise<void> {
  const conn = harness.connection
  const tx = new Transaction()
  tx.feePayer = payer.publicKey
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash
  tx.add(ix)
  await tx.sign(payer, ...signers)
  const sig = await conn.sendRawTransaction(await tx.serialize(), { skipPreflight: false })
  await harness.confirmSignature(sig)
}

export default globalSetup
