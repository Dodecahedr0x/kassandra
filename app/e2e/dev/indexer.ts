/**
 * `make dev` stack — indexer readiness + active-market price-history seeding.
 * Pure move/extract from `dev-full.ts`.
 */
import { indexerUrl } from './env.ts'
import { log } from './teardown.ts'
import type { SeedCtx } from '../seed.ts'
import type { ActiveMarketSeed } from '../seed-market.ts'
import { swapOnPool } from '../seed-market-active.ts'

/** Wait until the indexer's /status reports it has crawled `minEvents`. */
export async function waitForIndexer(minEvents: number, timeoutMs = 60_000): Promise<void> {
  const deadline = Date.now() + timeoutMs
  let last = ''
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${indexerUrl}/status`)
      if (res.ok) {
        const s = (await res.json()) as { eventCount: number }
        last = JSON.stringify(s)
        if (s.eventCount >= minEvents) return
      }
    } catch {
      /* still starting */
    }
    await new Promise((r) => setTimeout(r, 500))
  }
  log(`[dev] ⚠ indexer did not reach ${minEvents} events in ${timeoutMs}ms (last: ${last}) — continuing`)
}

/**
 * Give the active market a non-trivial price history: wait until the indexer's
 * price subscriber is live (its baseline candle exists), then drive a few swaps
 * that move the pool up and down. Each swap's pool update is recorded as a candle
 * point, so the market's chart shows genuine movement in `make dev`.
 */
export async function seedActivePriceHistory(ctx: SeedCtx, seed: ActiveMarketSeed): Promise<void> {
  const candlesUrl = `${indexerUrl}/api/markets/${seed.market}/candles?interval=60&limit=5`
  const deadline = Date.now() + 30_000
  // 1) Wait until the subscriber has captured its baseline point (⇒ it's subscribed).
  for (;;) {
    try {
      const res = await fetch(candlesUrl)
      if (res.ok && ((await res.json()) as unknown[]).length >= 1) break
    } catch {
      /* indexer/subscriber still coming up */
    }
    if (Date.now() > deadline) throw new Error('price subscriber did not produce a baseline candle')
    await new Promise((r) => setTimeout(r, 500))
  }
  // 2) Move the price both ways so the candle has a real range (down → up → down).
  log('[dev]   · driving swaps on the active pool (down → up → down) to populate the price chart')
  await swapOnPool(ctx, seed, 'down', 2_000_000_000n)
  await new Promise((r) => setTimeout(r, 1_200))
  await swapOnPool(ctx, seed, 'up', 3_000_000_000n)
  await new Promise((r) => setTimeout(r, 1_200))
  await swapOnPool(ctx, seed, 'down', 1_000_000_000n)
}
