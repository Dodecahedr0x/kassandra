import type { Market } from '@kassandra-market/oracles'
import type { AmmV04 } from '../../../../data/ammV04'
import { buildCrankTwapIxs, crankRateLimited, type Pool } from '../../../../data/actions/challengeTrade'
import { useWriteAction } from '../../../../hooks/useWriteAction'
import { ConnectGate } from '../ConnectGate'
import { SubmitButton } from '../formPrimitives'
import { WriteStatusRegion } from '../WriteStatusRegion'

/**
 * The CRANK sub-form: a permissionless per-pool button folding the current price
 * into the pool's TWAP observation → `buildCrankTwapIxs`. Disabled + hinted when
 * the pool was cranked within the last 150 slots (the on-chain rate limit).
 */
export function CrankForm({
  market,
  pool,
  amm,
  currentSlot,
  refetch,
}: {
  market: Market
  pool: Pool
  amm: AmmV04 | null
  currentSlot: bigint | null
  refetch: () => void
}) {
  const action = useWriteAction(refetch)
  const limited = crankRateLimited(amm, currentSlot)
  const label = pool === 'pass' ? 'Pass' : 'Fail'

  return (
    <ConnectGate connected={action.connected}>
      <form
        className="flex flex-col gap-2"
        onSubmit={(e) => {
          e.preventDefault()
          void action.run(() => buildCrankTwapIxs({ market, pool }))
        }}
        noValidate
      >
        <div className="flex items-center gap-3">
          <SubmitButton verb={`Crank ${label} TWAP`} status={action.status} disabled={limited} />
          {limited ? (
            <span className="font-inter text-[12px] text-bronze">
              Recently cranked — wait ~150 slots.
            </span>
          ) : null}
        </div>
        <WriteStatusRegion status={action.status} successVerb="Cranked" />
      </form>
    </ConnectGate>
  )
}
