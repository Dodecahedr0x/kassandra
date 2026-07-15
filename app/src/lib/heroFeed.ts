/**
 * Pure, side-effect-free selection + mapping for the landing hero's live cards:
 * the top-k oracles by stake and top-k markets by liquidity, mapped into a flat
 * {@link HeroCard} view model and interleaved (oracle, market, oracle, …). No
 * React, no fetching — the Hero component feeds in what `useOracles()` /
 * `useMarkets()` / `useOracleMeta()` already loaded, so this stays unit-testable
 * offline. Subjects come from the batched oracle-meta map (keyed by the oracle
 * PDA; a market keys off its linked `market.oracle`).
 */
import type { OracleSummary } from '../data/oracles'
import type { MarketSummary } from '../market/data/markets'
import type { OracleMetaView } from '../hooks/useOracleMeta'
import { oracleBonds } from './oracleStats'
import { phaseView, formatKass, truncateMiddle } from './oracleView'
import { impliedYesProbability, formatProbability, statusLabel, statusTone } from '../market/lib/marketView'

/** Chip tone set spanning both the oracle phase tones and the market status tones. */
export type HeroTone = 'neutral' | 'info' | 'accent' | 'ember' | 'confirmed' | 'muted'

/** Flat view model for one hero constellation card (real or example). */
export interface HeroCard {
  /** Stable React key + de-dupe id (the PDA for real cards). */
  id: string
  kind: 'oracle' | 'market'
  /** The linked oracle PDA — the card's own pubkey for an oracle, `market.oracle` for a market. */
  oracleId: string
  /** Route to the detail (real) or browse (example) view. */
  href: string
  /** The question — the oracle subject, or a fallback when meta is missing. */
  title: string
  /** Phase (oracle) or status (market) label for the chip. */
  status: string
  tone: HeroTone
  /** The highlighted figure (stake, or YES probability). */
  metricAccent: string
  /** The muted trailing text after the accent. */
  metricLabel: string
}

/** Default number of each kind to feature. */
export const HERO_K = 3

function descBig(a: bigint, b: bigint): number {
  return a < b ? 1 : a > b ? -1 : 0
}

/** Top-k oracles by stake (bond pool + dispute bond + staked total), descending. */
export function rankOracles(oracles: readonly OracleSummary[], k = HERO_K): OracleSummary[] {
  return [...oracles].sort((a, b) => descBig(oracleBonds(a.oracle), oracleBonds(b.oracle))).slice(0, k)
}

/** Top-k markets by total liquidity contributed, descending. */
export function rankMarkets(markets: readonly MarketSummary[], k = HERO_K): MarketSummary[] {
  return [...markets]
    .sort((a, b) => descBig(a.market.totalContributed, b.market.totalContributed))
    .slice(0, k)
}

/** The oracle PDAs whose subject we need for the featured cards (oracles + each market's oracle). */
export function metaKeysFor(
  oracles: readonly OracleSummary[],
  markets: readonly MarketSummary[],
  k = HERO_K,
): string[] {
  const keys = [
    ...rankOracles(oracles, k).map((o) => o.pubkey),
    ...rankMarkets(markets, k).map((m) => m.market.oracle.toString()),
  ]
  return [...new Set(keys)]
}

function oracleCard(o: OracleSummary, meta: Map<string, OracleMetaView>): HeroCard {
  const pv = phaseView(o.oracle.phase)
  return {
    id: o.pubkey,
    kind: 'oracle',
    oracleId: o.pubkey,
    href: `/oracles/${o.pubkey}`,
    title: meta.get(o.pubkey)?.subject?.trim() || `Oracle ${truncateMiddle(o.pubkey)}`,
    status: pv.label,
    tone: pv.tone as HeroTone,
    metricAccent: `${formatKass(oracleBonds(o.oracle))} KASS`,
    metricLabel: 'at stake',
  }
}

function marketCard(m: MarketSummary, meta: Map<string, OracleMetaView>): HeroCard {
  const prob = formatProbability(impliedYesProbability(m.reserves))
  const liquidity = `${formatKass(m.market.totalContributed)} KASS`
  const hasProb = prob !== '—'
  return {
    id: m.pubkey,
    kind: 'market',
    oracleId: m.market.oracle.toString(),
    href: `/markets/${m.pubkey}`,
    title: meta.get(m.market.oracle.toString())?.subject?.trim() || `Market ${truncateMiddle(m.pubkey)}`,
    status: statusLabel(m.market.status),
    tone: statusTone(m.market.status) as HeroTone,
    metricAccent: hasProb ? `YES ${prob}` : liquidity,
    metricLabel: hasProb ? `· ${liquidity} liq.` : 'liquidity',
  }
}

/** Interleave two lists: a0, b0, a1, b1, … then any remaining tail. */
export function interleave<T>(a: readonly T[], b: readonly T[]): T[] {
  const out: T[] = []
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if (i < a.length) out.push(a[i])
    if (i < b.length) out.push(b[i])
  }
  return out
}

/**
 * Build the interleaved live hero cards from loaded data. Empty in, empty out —
 * the Hero component pads/falls back to example cards so the constellation is
 * never empty while data loads or on a cluster with no oracles/markets.
 */
export function buildHeroCards(
  oracles: readonly OracleSummary[],
  markets: readonly MarketSummary[],
  meta: Map<string, OracleMetaView>,
  k = HERO_K,
): HeroCard[] {
  return interleave(
    rankOracles(oracles, k).map((o) => oracleCard(o, meta)),
    rankMarkets(markets, k).map((m) => marketCard(m, meta)),
  )
}

/**
 * Slot-index pairs `[oracleIndex, marketIndex]` for each displayed market whose
 * linked oracle is ALSO a displayed oracle — the constellation draws a connector
 * between those two cards. Indices are positions in `cards` (= scatter slots).
 */
export function heroConnections(cards: readonly HeroCard[]): [number, number][] {
  const oracleSlot = new Map<string, number>()
  cards.forEach((c, i) => {
    if (c.kind === 'oracle') oracleSlot.set(c.oracleId, i)
  })
  const pairs: [number, number][] = []
  cards.forEach((c, i) => {
    if (c.kind !== 'market') return
    const oracleIndex = oracleSlot.get(c.oracleId)
    if (oracleIndex !== undefined) pairs.push([oracleIndex, i])
  })
  return pairs
}
