/**
 * RF4 GATED FORKED-MAINNET surfpool CHALLENGE + AI-CLAIM E2E (`KASSANDRA_E2E=1`).
 *
 * Proves the RF4 challenge action layer end-to-end against a surfpool validator
 * FORKING MAINNET (so MetaDAO's DEPLOYED programs — conditional_vault `VLTX…`,
 * amm `AMMyu…`, futarchy `FUTAREL…` — are lazily fetched + EXECUTABLE), in
 * `clock` block-production mode with a fast slot-time (the v0.4 AMM crank
 * rate-limit is SLOT-based). This mirrors the PROVEN SDK recipe
 * (`sdks/oracles/ts/test/surfpool/challenge-market-e2e.test.ts`) wholesale, but drives the
 * THREE app builders through the app's {@link keypairSender}/{@link sendAndConfirm}
 * seam:
 *
 *   buildSubmitAiClaimIxs → each proposer stamps its AI claim in the AiClaim
 *                           phase; the AiClaim PDA appears on-chain (decoded).
 *   buildOpenChallengeIxs → a Market + USDC escrow open against the composed
 *                           MetaDAO market (program-signed split_tokens CPI runs
 *                           on the forked conditional_vault); the Market PDA +
 *                           ai_claim.challenged flip are asserted.
 *   buildSettleFromMarketIxs → after a REAL swap-driven FAIL-pool TWAP clears the
 *                           10% margin (two cranks ≥150 slots apart), the SD1
 *                           derive-from-Market settle builder derives the full
 *                           15-account settle set from the DECODED on-chain Market
 *                           + Oracle (NOT the composed JSON), resolves the question
 *                           FAIL-side and DISQUALIFIES + slashes the proposer; the
 *                           economics are asserted.
 *
 * DRIVEN LIVE: submitAiClaim + openChallenge (RF4 builders) + the SD1 one-click
 * derive-from-Market settle (`buildSettleFromMarketIxs`).
 * The AMM pool build/swap/crank is SETUP via the SDK `ammV04` builders (the crank
 * is not an app builder); the market composition (question + conditional vaults)
 * uses the same raw CPIs the SDK test documents.
 *
 * Gated: skips (never fails) unless `KASSANDRA_E2E=1` AND surfpool + the `.so`
 * are present. The fork needs network (mainnet datasource) + is slower.
 */
import { Keypair } from "@solana/web3.js";
import { buildDaoBlob } from "../../sdks/oracles/ts/test/surfpool/futarchy-dao.ts";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  Phase,
  TOKEN_PROGRAM_ID,
  EXTERNAL_PROGRAM_IDS,
  futarchy,
  decodeAiClaim,
  decodeMarket,
  decodeOracle,
  decodeProposer,
  initProtocol,
  setGovernance,
} from "@kassandra-market/oracles";
import * as pda from "@kassandra-market/oracles";

import {
  SurfpoolHarness,
  mintBytes,
  surfpoolReady,
  toHex,
} from "../../sdks/oracles/ts/test/surfpool/harness.ts";
import {
  BOND,
  type Fixture,
  buildPool,
  crankPool,
  decodeAmmTwap,
  fetchAccount,
  questionResolution,
  sendIx,
  swapBuy,
  tokenBalance,
} from "./helpers/challengeE2eHarness.ts";
import {
  composeMarket,
  frontDoorToChallenge,
  openChallengeViaApp,
  settleChallengeViaApp,
} from "./helpers/challengeE2eDrivers.ts";

const ENABLED = process.env.KASSANDRA_E2E === "1" && surfpoolReady();

const FUTARCHY_ID = EXTERNAL_PROGRAM_IDS.futarchyV06;

const KASS_PRICE_TWAP = 500_000_000n;
const KASS_PRICE_SCALE = 1_000_000_000_000n;

const BASE_RESERVE = 100_000_000_000n;
const QUOTE_NEUTRAL = 100_000_000n;

describe.skipIf(!ENABLED)("RF4 challenge/ai-claim action layer over FORKED MetaDAO", () => {
  let f: Fixture;

  beforeAll(async () => {
    // Fork mainnet + clock block-production (fast slot-time) — the v0.4 AMM crank
    // is SLOT-based. Dedicated port (8940) so it never collides with the other
    // gated suites (finalize 8901 / claims 8931 / SDK challenge 8920).
    const harness = await SurfpoolHarness.start({
      port: 8940,
      fork: "mainnet",
      blockProductionMode: "clock",
      slotTimeMs: 10,
      readyTimeoutMs: 60_000,
    });
    const payer = await Keypair.generate();
    await harness.airdrop(payer.publicKey.toString(), 1_000_000_000_000);

    const mintAuth = await pda.mintAuthority();
    const kassMint = await Keypair.generate();
    const usdcMint = await Keypair.generate();
    await harness.setAccount(kassMint.publicKey.toString(), {
      lamports: 1_000_000_000,
      owner: TOKEN_PROGRAM_ID.toString(),
      executable: false,
      data: toHex(mintBytes(mintAuth.address.toBytes(), 10n ** 18n, 9)),
    });
    await harness.setAccount(usdcMint.publicKey.toString(), {
      lamports: 1_000_000_000,
      owner: TOKEN_PROGRAM_ID.toString(),
      executable: false,
      data: toHex(mintBytes(payer.publicKey.toBytes(), 10n ** 18n, 6)),
    });

    const kassDao = (await Keypair.generate()).publicKey;
    await harness.setAccount(kassDao.toString(), {
      lamports: 5_000_000,
      owner: FUTARCHY_ID.toString(),
      executable: false,
      data: toHex(buildDaoBlob(KASS_PRICE_TWAP * 1_000_000n, 1_000_000n, 0n, 0)),
    });

    f = { harness, payer, kassMint, usdcMint, kassDao };

    await sendIx(f, await initProtocol({
      admin: payer.publicKey,
      kassMint: kassMint.publicKey,
      usdcMint: usdcMint.publicKey,
    }));
    const multisig = (await futarchy.pda.squadsMultisig(kassDao)).address;
    const daoAuthority = (await futarchy.pda.squadsVault(multisig, 0)).address;
    await sendIx(f, await setGovernance({ authority: payer.publicKey, daoAuthority, kassDao }));
  }, 120_000);

  afterAll(async () => {
    await f?.harness.teardown();
  });

  it("DISQUALIFY: submitAiClaim → openChallenge → swap-driven TWAP crank → settle (via the app builders)", async () => {
    const nonce = 200n;

    // ---- drive to Challenge; the AiClaims are stamped via the APP builder -----
    const c = await frontDoorToChallenge(f, nonce);
    expect(decodeOracle(await fetchAccount(f, c.oracle)).phase).toBe(Phase.Challenge);

    // Each proposer's AiClaim PDA is live on-chain (submitAiClaim driven live).
    for (const proposer of c.proposerPdas) {
      const aiClaim = (await pda.aiClaim(c.oracle, proposer)).address;
      const decoded = decodeAiClaim(await fetchAccount(f, aiClaim));
      expect(decoded.proposer.toString()).toBe(proposer.toString());
      expect(Array.from(decoded.modelId)).toEqual(Array.from(new Uint8Array(32).fill(0xa1)));
    }

    const market = await composeMarket(f, c.oracle);

    // ---- REAL pass/fail v0.4 AMM pools; FAIL gets a genuine BUY swap ----------
    const passAmm = await buildPool(f, market.kass.passMint, market.usdc.passMint, BASE_RESERVE, QUOTE_NEUTRAL);
    const failAmm = await buildPool(f, market.kass.failMint, market.usdc.failMint, BASE_RESERVE, QUOTE_NEUTRAL);
    await crankPool(f, passAmm);
    await swapBuy(f, market.kass.failMint, market.usdc.failMint, 90_000_000n);
    await crankPool(f, failAmm);
    await crankPool(f, failAmm);

    const passTwap = decodeAmmTwap(await fetchAccount(f, passAmm)).twap;
    const failTwap = decodeAmmTwap(await fetchAccount(f, failAmm)).twap;
    expect(passTwap, "pass TWAP must be a real non-zero observation").toBeGreaterThan(0n);
    expect(failTwap * 10n, "fail*DEN must clear pass*(DEN+NUM)").toBeGreaterThan(passTwap * 11n);

    // ================= openChallenge via the APP builder =======================
    const challenger = await openChallengeViaApp(f, nonce, c, market, passAmm, failAmm);
    const marketPda = (await pda.market(c.aiClaim)).address;

    const m = decodeMarket(await fetchAccount(f, marketPda));
    expect(m.oracle.toString()).toBe(c.oracle.toString());
    expect(m.proposer.toString()).toBe(c.proposer.toString());
    expect(m.challenger.toString()).toBe(challenger.publicKey.toString());
    expect(m.question.toString()).toBe(market.question.toString());
    expect(m.kassVault.toString()).toBe(market.kass.vault.toString());
    expect(decodeAiClaim(await fetchAccount(f, c.aiClaim)).challenged).toBe(true);
    expect(decodeOracle(await fetchAccount(f, c.oracle)).openChallengeCount).toBe(1);

    const escrow = (await pda.challengeUsdcVault(marketPda)).address;
    const requiredUsdc = (BOND * KASS_PRICE_TWAP) / KASS_PRICE_SCALE;
    expect(await tokenBalance(f, escrow)).toBe(requiredUsdc);
    expect(m.challengerUsdc).toBe(requiredUsdc);
    expect(await tokenBalance(f, market.oraclePassKass)).toBe(BOND);
    expect(await tokenBalance(f, market.oracleFailKass)).toBe(BOND);

    // ================= settleChallenge via the APP builder =====================
    const oBefore = decodeOracle(await fetchAccount(f, c.oracle));
    const stakeVault = (await pda.stakeVault(c.oracle)).address;
    const stakeBefore = await tokenBalance(f, stakeVault);

    const payouts = await settleChallengeViaApp(f, nonce, c, market, marketPda, challenger, passAmm, failAmm);

    const escrowAmt = (BOND * KASS_PRICE_TWAP) / KASS_PRICE_SCALE; // 500_000
    const kassFee = BOND / 100n;
    expect(questionResolution(await fetchAccount(f, market.question))).toEqual([0, 1]);
    expect(decodeMarket(await fetchAccount(f, marketPda)).settled).toBe(true);
    expect(decodeOracle(await fetchAccount(f, c.oracle)).openChallengeCount).toBe(0);
    const p = decodeProposer(await fetchAccount(f, c.proposer));
    expect(p.disqualified).toBe(true);
    expect(p.slashed).toBe(true);
    expect(p.slashedAmount).toBe(BOND - kassFee);
    const oAfter = decodeOracle(await fetchAccount(f, c.oracle));
    expect(oAfter.survivingCount).toBe(oBefore.survivingCount - 1);
    expect(oAfter.bondPool).toBe(oBefore.bondPool + (BOND - kassFee));
    expect(await tokenBalance(f, payouts.challengerKass)).toBe(kassFee);
    expect(await tokenBalance(f, stakeVault)).toBe(stakeBefore + (BOND - kassFee));
    expect(await tokenBalance(f, payouts.challengerUsdcDest)).toBe(escrowAmt);
    expect(await tokenBalance(f, payouts.proposerUsdc)).toBe(0n);
    expect(await tokenBalance(f, payouts.escrowVault)).toBe(0n);
  }, 300_000);
});
