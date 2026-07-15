import { useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from 'react'
import { Link } from 'react-router-dom'
import { Button, Reveal } from '../ui'
import { usePointerField } from '../../hooks/usePointerField'
import { useOracles } from '../../hooks/useOracles'
import { useMarkets } from '../../market/hooks/useMarkets'
import { useOracleMeta } from '../../hooks/useOracleMeta'
import {
  buildHeroCards,
  heroConnections,
  metaKeysFor,
  type HeroCard,
  type HeroTone,
} from '../../lib/heroFeed'

/** Desktop scatter slot: absolute placement + parallax drift depth (foreground = larger). */
const POSITIONS: { pos: string; depth: number }[] = [
  { pos: 'lg:top-[24px] lg:left-0', depth: 12 },
  { pos: 'lg:top-[8px] lg:right-[16px]', depth: 7 },
  { pos: 'lg:top-[286px] lg:left-0', depth: 9 },
  { pos: 'lg:top-[300px] lg:right-0', depth: 13 },
  { pos: 'lg:bottom-[8px] lg:left-[96px]', depth: 6 },
  { pos: 'lg:bottom-[28px] lg:right-[120px]', depth: 10 },
]

/** Chip color per tone — subtle Auros hairlines, ember reserved for the Challenge moment. */
const TONE_CLASSES: Record<HeroTone, string> = {
  neutral: 'border-pebble text-silver-mist',
  info: 'border-cyan-phosphor/30 text-cyan-phosphor',
  accent: 'border-lavender-phosphor/30 text-lavender-phosphor',
  ember: 'border-ember-orange/40 text-ember-orange',
  confirmed: 'border-chestnut/40 text-chestnut',
  muted: 'border-pebble text-stone',
}

/** A loading placeholder in a scatter slot — shown only while live data is still fetching. */
function SkeletonCard({ index }: { index: number }) {
  const slot = POSITIONS[index]
  return (
    <div className={'w-full lg:absolute lg:w-[248px] ' + slot.pos} aria-hidden="true">
      <div className="drift" style={{ '--drift-depth': `${slot.depth}px` } as CSSProperties}>
        <div className="animate-pulse rounded-card border border-pebble bg-pure-card p-4 motion-reduce:animate-none">
          <div className="flex items-center justify-between gap-2">
            <div className="h-2.5 w-14 rounded bg-white/10" />
            <div className="h-4 w-16 rounded-tag bg-white/10" />
          </div>
          <div className="mt-4 h-3 w-full rounded bg-white/10" />
          <div className="mt-2 h-3 w-2/3 rounded bg-white/10" />
          <div className="mt-3 h-3 w-1/2 rounded bg-white/10" />
        </div>
      </div>
    </div>
  )
}

function ConstellationCard({ card, index }: { card: HeroCard; index: number }) {
  const slot = POSITIONS[index]
  return (
    // Outer: absolute scatter position + staggered scroll-reveal entrance.
    // data-slot lets the connector overlay measure this card's base box.
    <Reveal className={'w-full lg:absolute lg:w-[248px] ' + slot.pos} delay={index * 90} data-slot={index}>
      {/* Inner: pointer parallax drift (kept off the reveal element so the two
          transforms don't clash). */}
      <div className="drift" style={{ '--drift-depth': `${slot.depth}px` } as CSSProperties}>
        <Link
          to={card.href}
          className="group block rounded-card border border-pebble bg-pure-card p-4 transition-[transform,border-color] duration-200 hover:-translate-y-1 hover:border-cyan-phosphor/40 focus-visible:outline-none focus-visible:border-cyan-phosphor/60"
        >
          <div className="flex items-center justify-between gap-2">
            <span className="label-eyebrow font-inter text-[11px] text-silver-mist">{card.kind}</span>
            <span
              className={
                'rounded-tag border px-2 py-0.5 font-inter text-[10px] font-medium uppercase tracking-[0.08em] ' +
                TONE_CLASSES[card.tone]
              }
            >
              {card.status}
            </span>
          </div>
          <p className="mt-3 line-clamp-2 font-inter text-[14px] leading-snug text-charcoal-bark">
            {card.title}
          </p>
          <p className="mt-2 font-inter text-[13px] font-medium tabular-nums text-bronze">
            <span className="text-lavender-phosphor">{card.metricAccent}</span> {card.metricLabel}
          </p>
        </Link>
      </div>
    </Reveal>
  )
}

/**
 * Hero — the signature Auros constellation, now backed by LIVE protocol data:
 * the top oracles by stake and top markets by liquidity, interleaved as scattered
 * cards (see heroFeed). A bioluminescent orb tracks the cursor and each card
 * drifts toward it at its own depth. When a displayed market uses a displayed
 * oracle, a connector line links the two. While the first fetch is in flight,
 * loading skeletons fill the slots (never fabricated cards). Desktop: cards are
 * absolutely scattered around the headline; mobile: a stacked grid. All motion is
 * transform/opacity only and disabled under prefers-reduced-motion.
 */
export default function Hero() {
  const fieldRef = usePointerField<HTMLElement>()
  const { data: oracles, loading: oraclesLoading } = useOracles()
  const { data: markets, loading: marketsLoading } = useMarkets()

  // Subjects for the featured accounts — one batched indexer call, keyed off the
  // ranked set so it refetches only when the top oracles/markets change.
  const metaKeys = useMemo(() => metaKeysFor(oracles ?? [], markets ?? []), [oracles, markets])
  const meta = useOracleMeta(metaKeys)

  // ONLY real accounts — the top oracles by stake + markets by liquidity,
  // interleaved (never fabricated example cards).
  const cards = useMemo(
    () => buildHeroCards(oracles ?? [], markets ?? [], meta).slice(0, 6),
    [oracles, markets, meta],
  )
  // While the first fetch is in flight and nothing has arrived yet, show loading
  // skeletons rather than fake content.
  const showSkeletons = cards.length === 0 && (oraclesLoading || marketsLoading)

  // Connect an oracle card to a market card when the market uses that oracle and
  // both are on screen. We measure the cards' BASE boxes (offsetLeft/Top ignore
  // the reveal/drift CSS transforms) and draw an SVG connector between centers.
  const containerRef = useRef<HTMLDivElement>(null)
  const connections = useMemo(() => heroConnections(cards), [cards])
  const [links, setLinks] = useState<{ x1: number; y1: number; x2: number; y2: number }[]>([])

  useLayoutEffect(() => {
    const container = containerRef.current
    if (!container || connections.length === 0) {
      setLinks([])
      return
    }
    const measure = () => {
      const slot = new Map<number, HTMLElement>()
      container.querySelectorAll<HTMLElement>('[data-slot]').forEach((el) => {
        slot.set(Number(el.dataset.slot), el)
      })
      const center = (el: HTMLElement) => ({
        x: el.offsetLeft + el.offsetWidth / 2,
        y: el.offsetTop + el.offsetHeight / 2,
      })
      const next: { x1: number; y1: number; x2: number; y2: number }[] = []
      for (const [o, m] of connections) {
        const oEl = slot.get(o)
        const mEl = slot.get(m)
        if (!oEl || !mEl) continue
        const a = center(oEl)
        const b = center(mEl)
        next.push({ x1: a.x, y1: a.y, x2: b.x, y2: b.y })
      }
      setLinks(next)
    }
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(container)
    return () => ro.disconnect()
  }, [connections])

  return (
    <section
      id="top"
      ref={fieldRef}
      aria-labelledby="hero-heading"
      className="relative overflow-hidden px-6 pt-16 pb-8 lg:pt-20"
    >
      {/* Bioluminescent cursor orb — atmospheric, non-interactive. Full-bleed
          across the section and clipped at the viewport edges (overflow-hidden
          above), so its soft falloff never shows a box edge mid-content. */}
      <div aria-hidden="true" className="cursor-orb pointer-events-none absolute inset-0 z-0" />

      <div ref={containerRef} className="relative z-10 mx-auto max-w-[1200px] lg:min-h-[680px]">
        {/* Connector overlay — thin bioluminescent lines linking each oracle card to
            the market that uses it (desktop scatter only; painted under the cards). */}
        {links.length > 0 ? (
          <svg
            aria-hidden="true"
            className="pointer-events-none absolute inset-0 hidden h-full w-full lg:block"
          >
            {links.map((l, i) => (
              <g key={i}>
                <line
                  x1={l.x1}
                  y1={l.y1}
                  x2={l.x2}
                  y2={l.y2}
                  stroke="rgba(43,214,199,0.28)"
                  strokeWidth={1.5}
                  strokeLinecap="round"
                />
                <line
                  x1={l.x1}
                  y1={l.y1}
                  x2={l.x2}
                  y2={l.y2}
                  className="hero-link-flow"
                  stroke="rgba(203,255,252,0.55)"
                  strokeWidth={1}
                  strokeLinecap="round"
                  strokeDasharray="1.5 9"
                />
                <circle cx={l.x1} cy={l.y1} r={2.5} fill="rgba(203,255,252,0.7)" />
                <circle cx={l.x2} cy={l.y2} r={2.5} fill="rgba(203,255,252,0.7)" />
              </g>
            ))}
          </svg>
        ) : null}

        {/* Headline layer — first in DOM (mobile order), centered overlay on desktop.
            Drifts gently OPPOSITE the cards (negative depth) for layered depth. */}
        <div className="relative z-10 mx-auto flex max-w-[680px] flex-col items-center text-center lg:absolute lg:inset-0 lg:justify-center">
          <div className="drift" style={{ '--drift-depth': '-4px' } as CSSProperties}>
            <h1
              id="hero-heading"
              className="font-serif font-light text-sepia text-[clamp(3rem,8vw,4rem)] leading-[1] tracking-[-0.03em]"
            >
              <span className="block">Truth,</span>
              <span className="block italic text-bronze">settled.</span>
            </h1>
            <p className="mt-6 max-w-[520px] font-inter text-[17px] leading-relaxed text-bronze">
              Kassandra is a decentralized, AI-assisted optimistic oracle on Solana: propose an
              answer, open a challenge window, and let anyone reproduce the verdict.
            </p>
            <div className="mt-8 flex flex-wrap items-center justify-center gap-4">
              <Button variant="PrimaryChestnut">Read the docs</Button>
              <Button
                variant="GhostOutline"
                onClick={() =>
                  document.getElementById('how-it-works')?.scrollIntoView({ behavior: 'smooth' })
                }
              >
                See how it works
              </Button>
            </div>
          </div>
        </div>

        {/* Cards layer — live oracles/markets only. Static grid on mobile, absolute scatter on desktop. */}
        <div
          aria-label="Featured live oracles and markets"
          aria-busy={showSkeletons}
          className="mt-12 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:mt-0 lg:block"
        >
          {cards.length > 0
            ? cards.map((card, i) => <ConstellationCard key={card.id} card={card} index={i} />)
            : showSkeletons
              ? POSITIONS.map((_, i) => <SkeletonCard key={i} index={i} />)
              : null}
        </div>
      </div>
    </section>
  )
}
