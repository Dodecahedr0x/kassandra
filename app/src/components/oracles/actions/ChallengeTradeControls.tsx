import { useEffect, useState } from 'react'
import type { Market, Oracle, Proposer } from '@kassandra-market/oracles'
import {
  PRICE_SCALE,
  marginProgress,
  twapPrice,
  willDisqualify,
} from '../../../data/ammV04'
import { useMarketAmms } from '../../../hooks/useMarketAmms'
import { useWriteAction } from '../../../hooks/useWriteAction'
import { isMockMode } from '../../../data/mockOracles'
import { relativeDeadline } from '../../../lib/oracleView'
import { Card } from '../../ui'
import { Chip } from '../Chip'
import { SwapForm } from './ChallengeTradeControls/SwapForm'
import { CrankForm } from './ChallengeTradeControls/CrankForm'
import { SettleButton } from './ChallengeTradeControls/SettleButton'

/** Progress at which the settle verdict is treated as "near" the disqualify margin. */
const NEAR_MARGIN = 0.85

/** Format a PRICE_SCALE-scaled TWAP (1e12) as a human decimal price string. */
function formatTwap(scaled: bigint | null): string {
  return scaled === null ? '—' : (Number(scaled) / Number(PRICE_SCALE)).toFixed(4)
}

/**
 * CU2 — the challenge-market TRADE / CRANK / SETTLE controls, rendered beside the
 * CU1 read viz in the detail's Challenge-market section. Three grouped write
 * surfaces over the same externally-composed MetaDAO v0.4 pools CU1 decodes:
 *
 *   - Swap: pool + side + amount + slippage, with a constant-product expected-out
 *     + price-impact preview from the CU1-decoded reserves;
 *   - Crank TWAP: permissionless per-pool, disabled/hinted when cranked within the
 *     on-chain 150-slot rate limit (uses the CU1 `lastUpdatedSlot` + current slot);
 *   - Settle: permissionless, enabled ONLY after `market.twapEnd` && !settled, with
 *     a live fail-vs-pass TWAP verdict preview (via CU1's `marginProgress`).
 *
 * The write controls are ConnectGate'd (the CU1 read viz stays visible
 * disconnected); phase gating is the caller's (only rendered in the Challenge
 * phase). The single ember accent is reserved for a genuine over-margin verdict
 * / high impact — no new embers beyond CU1's margin accent.
 */
export function ChallengeTradeControls({
  oraclePubkey,
  oracle,
  market,
  proposers,
  refetch,
}: {
  /** The oracle PDA (base58). */
  oraclePubkey: string
  /** The decoded oracle (its margin threshold drives the verdict preview). */
  oracle: Oracle
  /** The challenge {@link Market} being traded. */
  market: Market
  /**
   * The oracle's decoded proposers (keyed by PDA) — the one-click settle reads the
   * challenged proposer's `authority` (owner of the proposer USDC payout) off the
   * proposer whose pubkey == `market.proposer`.
   */
  proposers: { pubkey: string; proposer: Proposer }[]
  /** Refetch the oracle detail on a successful write. */
  refetch: () => void
}) {
  const { pass, fail, refetch: refetchAmms } = useMarketAmms(market)
  const [currentSlot, setCurrentSlot] = useState<bigint | null>(null)
  const { connection } = useWriteAction()

  // Best-effort current slot for the crank rate-limit hint (mock: skip).
  useEffect(() => {
    if (isMockMode()) return
    let active = true
    const getSlot = (connection as unknown as { getSlot?: () => Promise<number> }).getSlot
    if (typeof getSlot !== 'function') return
    getSlot.call(connection).then(
      (s: number) => {
        if (active) setCurrentSlot(BigInt(s))
      },
      () => {},
    )
    return () => {
      active = false
    }
  }, [connection, market.passAmm])

  const onWrite = () => {
    refetch()
    refetchAmms()
  }

  const passTwap = pass ? twapPrice(pass) : null
  const failTwap = fail ? twapPrice(fail) : null
  const progress = marginProgress(
    failTwap,
    passTwap,
    oracle.marketThresholdNum,
    oracle.marketThresholdDen,
  )
  const twapReady = passTwap !== null && failTwap !== null
  // The verdict uses the exact bigint on-chain boundary (strict `>`), not the
  // float `progress >= 1` — at exact equality the proposer survives on-chain.
  const wouldDisqualify =
    twapReady &&
    willDisqualify(
      failTwap,
      passTwap,
      oracle.marketThresholdNum,
      oracle.marketThresholdDen,
    )
  const near = twapReady && progress >= NEAR_MARGIN

  const nowUnix = BigInt(Math.floor(Date.now() / 1000))
  const settleOpen = !market.settled && nowUnix >= market.twapEnd

  // The challenged proposer's wallet authority (owner of the proposer USDC payout
  // the settle handler asserts) — off the proposer whose PDA == market.proposer.
  const marketProposer = market.proposer.toString()
  const proposerAuthority = proposers
    .find((p) => p.pubkey === marketProposer)
    ?.proposer.authority.toString()

  return (
    <Card className="mt-4 flex flex-col gap-5">
      <div>
        <h3 className="font-serif text-subheading font-light text-sepia">Trade &amp; settle</h3>
        <p className="mt-1 font-inter text-[13px] text-driftwood">
          Trade the pass/fail conditional pools, crank their TWAP, and settle the challenge — the
          swap-driven TWAP is what decides the verdict.
        </p>
      </div>

      {market.settled ? (
        <p className="font-inter text-[13px] text-bronze">
          This market is settled — trading and cranking are closed.
        </p>
      ) : (
        <>
          {/* Swap */}
          <section className="border-t border-pebble pt-4">
            <h4 className="font-inter text-[13px] font-medium text-sepia">Swap a pool</h4>
            <div className="mt-3">
              <SwapForm market={market} pools={{ pass, fail }} refetch={onWrite} />
            </div>
          </section>

          {/* Crank */}
          <section className="border-t border-pebble pt-4">
            <h4 className="font-inter text-[13px] font-medium text-sepia">Crank TWAP</h4>
            <p className="mt-0.5 font-inter text-[12px] text-driftwood">
              Permissionless — folds the current price into a pool&apos;s TWAP (once per ~150 slots).
            </p>
            <div className="mt-3 grid grid-cols-1 gap-4 sm:grid-cols-2">
              <CrankForm
                market={market}
                pool="pass"
                amm={pass}
                currentSlot={currentSlot}
                refetch={onWrite}
              />
              <CrankForm
                market={market}
                pool="fail"
                amm={fail}
                currentSlot={currentSlot}
                refetch={onWrite}
              />
            </div>
          </section>
        </>
      )}

      {/* Settle + verdict preview */}
      <section className="border-t border-pebble pt-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h4 className="font-inter text-[13px] font-medium text-sepia">Settle challenge</h4>
          {twapReady ? (
            <Chip tone={wouldDisqualify ? 'ember' : 'confirmed'}>
              {wouldDisqualify ? 'Would DISQUALIFY' : 'Would SURVIVE'}
            </Chip>
          ) : null}
        </div>

        {/* Verdict preview (CU1 marginProgress against the on-chain threshold). */}
        <div className="mt-2 rounded-tag border border-pebble bg-pure-card px-3 py-2 font-inter text-[12px]">
          <dl className="flex flex-col gap-1">
            <div className="flex items-baseline justify-between gap-3">
              <dt className="text-driftwood">Pass TWAP</dt>
              <dd className="tabular-nums text-sepia">{formatTwap(passTwap)}</dd>
            </div>
            <div className="flex items-baseline justify-between gap-3">
              <dt className="text-driftwood">Fail TWAP</dt>
              <dd className="tabular-nums text-sepia">{formatTwap(failTwap)}</dd>
            </div>
            <div className="flex items-baseline justify-between gap-3">
              <dt className="text-driftwood">
                Margin {oracle.marketThresholdNum.toString()}/{oracle.marketThresholdDen.toString()}
              </dt>
              <dd className={`tabular-nums ${near ? 'text-ember-orange' : 'text-sepia'}`}>
                {twapReady ? `${Math.round(progress * 100)}%` : '—'}
              </dd>
            </div>
          </dl>
          <p className="mt-1.5 text-driftwood">
            {!twapReady
              ? 'TWAP forming — the verdict is not yet meaningful (pre start-delay).'
              : wouldDisqualify
                ? 'The fail TWAP has cleared the margin — settling would disqualify the proposer.'
                : 'The fail TWAP is within the margin — settling would let the proposer survive.'}
          </p>
        </div>

        {market.settled ? (
          <p className="mt-3 font-inter text-[12px] text-driftwood">This market is already settled.</p>
        ) : !settleOpen ? (
          <p className="mt-3 font-inter text-[12px] text-bronze">
            Settle opens after the market&apos;s TWAP window ({relativeDeadline(market.twapEnd)}).
          </p>
        ) : (
          <div className="mt-3">
            <p className="font-inter text-[12px] text-driftwood">
              Permissionless — any connected wallet can crank the settle. The full account set is
              derived from the market; no paste needed.
            </p>
            <div className="mt-3">
              <SettleButton
                oracleKey={oraclePubkey}
                oracle={oracle}
                market={market}
                proposerAuthority={proposerAuthority}
                refetch={onWrite}
              />
            </div>
          </div>
        )}
      </section>
    </Card>
  )
}

export default ChallengeTradeControls
