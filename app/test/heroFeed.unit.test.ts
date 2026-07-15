/**
 * Offline unit tests for `src/lib/heroFeed.ts` — the pure ranking + view-model
 * mapping behind the landing hero's live cards (top-k oracles by stake, top-k
 * markets by liquidity, interleaved). No React / chain: oracle & market summaries
 * are minimal partials cast to the read types.
 */
import { describe, expect, it } from 'vitest'
import { Phase } from '@kassandra-market/oracles'

import type { OracleSummary } from '../src/data/oracles'
import type { MarketSummary } from '../src/market/data/markets'
import type { OracleMetaView } from '../src/hooks/useOracleMeta'
import {
  buildHeroCards,
  heroConnections,
  interleave,
  metaKeysFor,
  rankMarkets,
  rankOracles,
  type HeroCard,
} from '../src/lib/heroFeed'

function oracle(pubkey: string, stake: bigint, phase: Phase = Phase.Proposal): OracleSummary {
  return {
    pubkey,
    oracle: { bondPool: stake, disputeBondTotal: 0n, totalOracleStake: 0n, phase },
  } as unknown as OracleSummary
}

function market(pubkey: string, oraclePk: string, liquidity: bigint): MarketSummary {
  return {
    pubkey,
    reserves: null,
    market: { oracle: { toString: () => oraclePk }, totalContributed: liquidity, status: 0 },
  } as unknown as MarketSummary
}

const meta = new Map<string, OracleMetaView>([
  ['o-big', { subject: 'Did protocol X ship mainnet by Jun 30?' }],
  ['mo-1', { subject: 'Will the grant milestone verify on-chain?' }],
])

describe('rankOracles / rankMarkets', () => {
  it('takes the top-k by stake / liquidity, descending', () => {
    const oracles = [oracle('o-sm', 10n), oracle('o-big', 900n), oracle('o-md', 100n)]
    expect(rankOracles(oracles, 2).map((o) => o.pubkey)).toEqual(['o-big', 'o-md'])

    const markets = [market('m-sm', 'x', 5n), market('m-big', 'y', 500n)]
    expect(rankMarkets(markets, 1).map((m) => m.pubkey)).toEqual(['m-big'])
  })

  it('does not mutate the input array', () => {
    const oracles = [oracle('a', 1n), oracle('b', 2n)]
    rankOracles(oracles)
    expect(oracles.map((o) => o.pubkey)).toEqual(['a', 'b'])
  })
})

describe('metaKeysFor', () => {
  it('collects the ranked oracle PDAs plus each ranked market oracle, de-duped', () => {
    const oracles = [oracle('o-big', 900n)]
    const markets = [market('m1', 'mo-1', 500n)]
    expect(metaKeysFor(oracles, markets, 3).sort()).toEqual(['mo-1', 'o-big'])
  })
})

describe('interleave', () => {
  it('alternates then appends the longer tail', () => {
    expect(interleave([1, 3, 5], [2, 4])).toEqual([1, 2, 3, 4, 5])
    expect(interleave(['a'], ['b', 'c', 'd'])).toEqual(['a', 'b', 'c', 'd'])
  })
})

describe('buildHeroCards', () => {
  it('interleaves oracle/market cards with subject + stake/liquidity metrics', () => {
    const cards = buildHeroCards([oracle('o-big', 900_000_000n)], [market('m1', 'mo-1', 12_000_000_000n)], meta)
    expect(cards.map((c) => c.kind)).toEqual(['oracle', 'market'])
    expect(cards[0]).toMatchObject({
      id: 'o-big',
      href: '/oracles/o-big',
      title: 'Did protocol X ship mainnet by Jun 30?',
      metricAccent: '0.9 KASS',
      metricLabel: 'at stake',
    })
    // market with null reserves → no probability, falls back to liquidity figure
    expect(cards[1]).toMatchObject({
      kind: 'market',
      href: '/markets/m1',
      title: 'Will the grant milestone verify on-chain?',
      metricAccent: '12 KASS',
      metricLabel: 'liquidity',
    })
  })

  it('falls back to the real account id when meta has no subject', () => {
    const cards = buildHeroCards([oracle('OracleWithNoSubject01', 1n)], [], new Map())
    expect(cards[0].title).toMatch(/^Oracle /)
    expect(cards[0].title).toContain('…') // truncated real pubkey, not a generic sentence
  })

  it('tags each card with its linked oracle id', () => {
    const cards = buildHeroCards([oracle('o-1', 1n)], [market('m-1', 'o-1', 1n)], new Map())
    expect(cards[0]).toMatchObject({ kind: 'oracle', oracleId: 'o-1' })
    expect(cards[1]).toMatchObject({ kind: 'market', oracleId: 'o-1' })
  })
})

describe('heroConnections', () => {
  const card = (id: string, kind: 'oracle' | 'market', oracleId: string): HeroCard =>
    ({ id, kind, oracleId }) as HeroCard

  it('pairs a market with its oracle when both are displayed', () => {
    // slots: 0 oracle(o-1), 1 market(→o-1), 2 oracle(o-2), 3 market(→zzz)
    const cards = [
      card('o-1', 'oracle', 'o-1'),
      card('m-1', 'market', 'o-1'),
      card('o-2', 'oracle', 'o-2'),
      card('m-2', 'market', 'zzz'), // oracle not displayed → no connection
    ]
    expect(heroConnections(cards)).toEqual([[0, 1]])
  })

  it('connects one oracle to multiple of its displayed markets', () => {
    const cards = [
      card('o-1', 'oracle', 'o-1'),
      card('m-a', 'market', 'o-1'),
      card('m-b', 'market', 'o-1'),
    ]
    expect(heroConnections(cards)).toEqual([
      [0, 1],
      [0, 2],
    ])
  })

  it('returns nothing when no market oracle is displayed', () => {
    expect(heroConnections([card('m-1', 'market', 'o-1')])).toEqual([])
  })
})
