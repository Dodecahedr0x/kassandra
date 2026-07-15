import { useMemo } from 'react'
import { useParams } from 'react-router-dom'
import { Phase } from '@kassandra-market/oracles'
import { Button, Card, EyebrowTag } from '../../components/ui'
import { PhaseChip } from '../../components/oracles/PhaseChip'
import { PhaseTimeline } from '../../components/oracles/PhaseTimeline'
import { EconomicPanel } from '../../components/oracles/EconomicPanel'
import { ChallengeMarketPanel } from '../../components/oracles/ChallengeMarketPanel'
import { ChallengeTradeControls } from '../../components/oracles/actions/ChallengeTradeControls'
import { Truncated } from '../../components/oracles/Truncated'
import { ActivityFeed } from '../../components/oracles/ActivityFeed'
import { OracleActions } from '../../components/oracles/actions'
import { isIndexerConfigured } from '../../data/indexer'
import { useOracleDetail } from '../../hooks/useOracles'
import { useOracleMeta } from '../../hooks/useOracleMeta'
import { OracleNotFoundError } from '../../data/oracles'
import { CLUSTER_LABELS, useCluster } from '../../lib/cluster'
import { RESOLVED_OPTION_NONE, formatKass, relativeDeadline, windowLabel } from '../../lib/oracleView'
import type { SettleCtx } from './helpers'
import { BackLink, Row, Section, Stat, VerdictBanner } from './primitives'
import { AiClaimCard, FactCard, MarketCard, ProposerCard } from './cards'

const emptyNote = (text: string) => (
  <p className="font-inter text-[14px] text-driftwood">{text}</p>
)

/**
 * The oracle detail view at `/oracles/:pubkey` — an editorial layout of one
 * decoded oracle + its facts, proposers, AI claims and challenge market
 * (consumes the FA2 data layer via `useOracleDetail`). Read-only. Loading /
 * error / not-found states.
 */
export default function OracleDetail() {
  const { pubkey } = useParams<{ pubkey: string }>()
  const { cluster } = useCluster()
  const search = typeof window !== 'undefined' ? window.location.search : ''
  const { data, loading, error, refetch } = useOracleDetail(pubkey)

  const notFound = error instanceof OracleNotFoundError

  return (
    <main className="mx-auto max-w-[1000px] px-6 py-16 md:py-20">
      <BackLink search={search} />

      {loading ? (
        <p className="mt-10 font-inter text-[15px] text-bronze" role="status">
          Reading the chain…
        </p>
      ) : notFound ? (
        <div className="mt-10 max-w-[560px]">
          <Card>
            <h1 className="font-serif text-heading-sm font-light text-sepia">Oracle not found</h1>
            <p className="mt-2 font-inter text-[15px] text-bronze">
              No Kassandra oracle lives at this address on{' '}
              <span className="font-medium text-sepia">{CLUSTER_LABELS[cluster]}</span>.
            </p>
            <p className="mt-2 break-all font-mono text-[12px] text-driftwood">{pubkey}</p>
          </Card>
        </div>
      ) : error ? (
        <div className="mt-10 max-w-[560px]">
          <Card>
            <h1 className="font-serif text-heading-sm font-light text-sepia">
              Couldn’t load this oracle
            </h1>
            <p className="mt-2 font-inter text-[15px] text-bronze">{error.message}</p>
            <div className="mt-5">
              <Button variant="GhostOutline" onClick={refetch}>
                Retry
              </Button>
            </div>
          </Card>
        </div>
      ) : data ? (
        <OracleBody detail={data} refetch={refetch} />
      ) : null}
    </main>
  )
}

/** The loaded oracle, laid out editorially. Split out so the states above stay readable. */
function OracleBody({
  detail,
  refetch,
}: {
  detail: NonNullable<ReturnType<typeof useOracleDetail>['data']>
  refetch: () => void
}) {
  const { pubkey, oracle, facts, proposers, aiClaims, market } = detail
  // On-chain plaintext subject + option labels (indexed from oracle_meta).
  const metaItems = useMemo(() => [pubkey], [pubkey])
  const meta = useOracleMeta(metaItems).get(pubkey)
  const options = meta?.options ?? []
  const resolved = oracle.phase === Phase.Resolved
  const hasResolvedOption = resolved && oracle.resolvedOption !== RESOLVED_OPTION_NONE
  const votingOpen = oracle.phase === Phase.FactVoting
  // The trade/crank/settle controls live only while the challenge round is open.
  const tradeOpen = oracle.phase === Phase.Challenge || oracle.phase === Phase.FinalRecompute
  // Terminal phases open the claim / close / sweep payout controls.
  const settleOpen = oracle.phase === Phase.Resolved || oracle.phase === Phase.InvalidDeadend
  const settle: SettleCtx | undefined = settleOpen
    ? { oracle: pubkey, kassMint: oracle.kassMint, refetch }
    : undefined

  return (
    <>
      {/* Header — the SUBJECT (verified question) + its options lead. */}
      <header className="mt-8">
        <EyebrowTag pill>Oracle</EyebrowTag>
        <h1 className="mt-3 font-serif text-heading font-light text-sepia">
          {meta?.subject ?? 'Oracle dispute'}
        </h1>
        {options.length > 0 && (
          <div className="mt-4 flex flex-wrap gap-2" aria-label="Options">
            {options.map((opt, i) => (
              <span
                key={i}
                className="rounded-tag border border-pebble bg-soft-cream px-2.5 py-1 font-inter text-[13px] text-bronze"
              >
                <span className="tabular-nums text-driftwood">{i}</span>
                <span className="mx-1 text-driftwood">·</span>
                {opt}
              </span>
            ))}
          </div>
        )}
        <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2 font-inter text-[13px] text-driftwood">
          <PhaseChip phase={oracle.phase} />
          <span>{relativeDeadline(oracle.deadline)}</span>
          <Truncated value={pubkey} copyable label="oracle address" />
        </div>
        {meta?.uri && (
          <div className="mt-3 flex items-baseline gap-2 font-inter text-[13px] text-driftwood">
            <span>Metadata</span>
            <a
              href={meta.uri}
              target="_blank"
              rel="noreferrer"
              className="text-chestnut underline decoration-dotted underline-offset-2"
            >
              extended JSON
            </a>
            <span className="text-driftwood/70" title="sha256 committed on-chain">
              (hash-verified)
            </span>
          </div>
        )}
        {resolved ? (
          <p className="mt-3 font-inter text-[14px] text-chestnut">
            {hasResolvedOption
              ? `Resolved to option ${oracle.resolvedOption}`
              : 'Resolved with no valid option (dead-end)'}
          </p>
        ) : null}
      </header>

      {/* At-a-glance verdict (h2 banner) + lifecycle timeline — additive header. */}
      <VerdictBanner oracle={oracle} />
      <PhaseTimeline oracle={oracle} />

      {/* Stats */}
      <div className="mt-8 grid grid-cols-2 gap-4 sm:grid-cols-3">
        <Stat label="Options" value={oracle.optionsCount} />
        <Stat label="Proposers" value={oracle.proposerCount} />
        <Stat label="Surviving" value={oracle.survivingCount} />
        <Stat label="Facts" value={oracle.factCount} />
        <Stat label="Settled facts" value={oracle.settledCount} />
        <Stat label="Open challenges" value={oracle.openChallengeCount} />
      </div>

      {/* Bond pool (scaled KASS) */}
      <div className="mt-4">
        <Card>
          <div className="font-inter text-[11px] uppercase tracking-[0.06em] text-driftwood">
            Bond pool
          </div>
          <div className="mt-1 font-serif text-heading-sm font-light text-sepia">
            {formatKass(oracle.bondPool)} KASS
          </div>
          <p className="mt-1 font-inter text-[12px] text-driftwood">
            dispute-bond total {formatKass(oracle.disputeBondTotal)} KASS
          </p>
        </Card>
      </div>

      {/* Economic picture — flat proportion viz over the decoded economics. */}
      <EconomicPanel oracle={oracle} proposers={proposers.map((p) => p.proposer)} />

      {/* Participate — the wallet-signed write forms + permissionless finalize
          cranks, phase-gated (WF2/RF1). The finalize tails are the proposer /
          fact PDA pubkeys from the already-fetched detail. */}
      <OracleActions
        pubkey={pubkey}
        oracle={oracle}
        refetch={refetch}
        proposers={proposers.map((p) => p.pubkey)}
        facts={facts.map((f) => f.pubkey)}
        market={market}
      />

      {/* Parameters */}
      <Section title="Parameters">
        <Card>
          <dl className="flex flex-col">
            <Row term="Fact quorum">
              {oracle.thresholdNum.toString()} / {oracle.thresholdDen.toString()}
            </Row>
            <Row term="Market margin">
              {oracle.marketThresholdNum.toString()} / {oracle.marketThresholdDen.toString()}
            </Row>
            <Row term="Flip slash">
              {oracle.flipSlashNum.toString()} / {oracle.flipSlashDen.toString()}
            </Row>
            <Row term="Phase window">{windowLabel(oracle.phaseWindow)}</Row>
            <Row term="Proposal window">{windowLabel(oracle.proposalWindow)}</Row>
            <Row term="TWAP window">{windowLabel(oracle.twapWindow)}</Row>
          </dl>
        </Card>
      </Section>

      {/* Facts */}
      <Section title="Facts" count={facts.length}>
        {facts.length === 0 ? (
          emptyNote('No facts submitted for this oracle.')
        ) : (
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
            {facts.map((f) => (
              <FactCard
                key={f.pubkey}
                pubkey={f.pubkey}
                fact={f.fact}
                voting={
                  votingOpen ? { oracle: pubkey, kassMint: oracle.kassMint, refetch } : undefined
                }
                settle={settle}
              />
            ))}
          </div>
        )}
      </Section>

      {/* Proposers */}
      <Section title="Proposers" count={proposers.length}>
        {proposers.length === 0 ? (
          emptyNote('No proposers registered.')
        ) : (
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
            {proposers.map((p) => (
              <ProposerCard key={p.pubkey} pubkey={p.pubkey} proposer={p.proposer} settle={settle} />
            ))}
          </div>
        )}
      </Section>

      {/* AI Claims */}
      <Section title="AI claims" count={aiClaims.length}>
        {aiClaims.length === 0 ? (
          emptyNote('No AI claims submitted.')
        ) : (
          <div className="flex flex-col gap-4">
            {aiClaims.map((c) => (
              <AiClaimCard key={c.pubkey} pubkey={c.pubkey} aiClaim={c.aiClaim} settle={settle} />
            ))}
          </div>
        )}
      </Section>

      {/* Market — the existing card (accounts + close control) plus the CU1
          live visualization panel (prices / TWAP / margin / countdown), additive. */}
      <Section title="Challenge market">
        {market ? (
          <>
            <MarketCard pubkey={market.pubkey} market={market.market} settle={settle} />
            <ChallengeMarketPanel market={market.market} oracle={oracle} />
            {tradeOpen ? (
              <ChallengeTradeControls
                oraclePubkey={pubkey}
                oracle={oracle}
                market={market.market}
                proposers={proposers}
                refetch={refetch}
              />
            ) : null}
          </>
        ) : (
          emptyNote('No challenge market opened for this oracle.')
        )}
      </Section>

      {/* On-chain activity — indexed event history (renders only when the
          indexer backend is configured; otherwise absent). */}
      {isIndexerConfigured() ? (
        <Section title="Activity">
          <ActivityFeed oracle={pubkey} />
        </Section>
      ) : null}

      {/* Accounts */}
      <Section title="Accounts">
        <Card>
          <dl className="flex flex-col">
            <Row term="Creator">
              <Truncated value={oracle.creator.toString()} copyable label="creator" />
            </Row>
            <Row term="KASS mint">
              <Truncated value={oracle.kassMint.toString()} copyable label="KASS mint" />
            </Row>
            <Row term="USDC mint">
              <Truncated value={oracle.usdcMint.toString()} copyable label="USDC mint" />
            </Row>
            <Row term="Stake vault">
              <Truncated value={oracle.stakeVault.toString()} copyable label="stake vault" />
            </Row>
          </dl>
        </Card>
      </Section>
    </>
  )
}
