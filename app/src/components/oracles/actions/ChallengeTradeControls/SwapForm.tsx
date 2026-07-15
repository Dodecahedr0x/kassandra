import { useState, type FormEvent } from 'react'
import type { Market } from '@kassandra-market/oracles'
import type { AmmV04 } from '../../../../data/ammV04'
import {
  buildSwapIxs,
  swapEstimate,
  type Pool,
  type Side,
} from '../../../../data/actions/challengeTrade'
import { useWriteAction } from '../../../../hooks/useWriteAction'
import { KASS_DECIMALS, USDC_DECIMALS, formatUnits } from '../../../../lib/oracleView'
import { parseAmount } from '../amount'
import { ConnectGate } from '../ConnectGate'
import { Field, SubmitButton, TextInput } from '../formPrimitives'
import { WriteStatusRegion } from '../WriteStatusRegion'

/** Parse a slippage tolerance in percent (0..100) → basis points. */
function parseSlippageBps(raw: string): { bps: number; error?: string } {
  const t = raw.trim()
  if (t === '') return { bps: 50 } // default 0.5%
  const pct = Number(t)
  if (!Number.isFinite(pct) || pct < 0 || pct > 100) {
    return { bps: 50, error: 'Slippage must be 0–100%.' }
  }
  return { bps: Math.round(pct * 100) }
}

/**
 * The SWAP sub-form: choose pool + side + amountIn + slippage; preview the
 * expected out + price impact from the CU1-decoded reserves (constant-product);
 * submit → `buildSwapIxs` (wallet-signed). The chosen pool's decoded `AmmV04`
 * drives both the preview and the `minAmountOut` slippage floor.
 */
export function SwapForm({
  market,
  pools,
  refetch,
}: {
  market: Market
  pools: { pass: AmmV04 | null; fail: AmmV04 | null }
  refetch: () => void
}) {
  const action = useWriteAction(refetch)
  const [pool, setPool] = useState<Pool>('fail')
  const [side, setSide] = useState<Side>('buy')
  const [amountRaw, setAmountRaw] = useState('')
  const [slipRaw, setSlipRaw] = useState('0.5')

  const amm = pool === 'pass' ? pools.pass : pools.fail
  // buy = USDC(quote,6) → KASS(base,9); sell is the reverse. Scale entry by the
  // IN mint's decimals and the preview by the OUT mint's.
  const inDecimals = side === 'buy' ? USDC_DECIMALS : KASS_DECIMALS
  const outDecimals = side === 'buy' ? KASS_DECIMALS : USDC_DECIMALS
  const parsed = parseAmount(amountRaw, inDecimals)
  const slip = parseSlippageBps(slipRaw)
  const est = swapEstimate(amm, side, parsed.value ?? 0n)
  const inLabel = side === 'buy' ? 'USDC (quote)' : 'KASS (base)'
  const outLabel = side === 'buy' ? 'KASS (base)' : 'USDC (quote)'
  const impactPct = Math.round(est.impact * 1000) / 10

  const onSubmit = (e: FormEvent) => {
    e.preventDefault()
    if (parsed.error || slip.error || parsed.value === undefined) return
    void action.run(() =>
      buildSwapIxs({
        connection: action.connection,
        market,
        pool,
        side,
        amountIn: parsed.value!,
        user: action.address!,
        slippageBps: slip.bps,
        amm,
      }),
    )
  }

  return (
    <ConnectGate connected={action.connected}>
      <form className="flex flex-col gap-3" onSubmit={onSubmit} noValidate>
        <div className="grid grid-cols-2 gap-3">
          <label className="flex flex-col gap-1.5">
            <span className="font-inter text-[13px] font-medium text-sepia">Pool</span>
            <select
              aria-label="Pool"
              value={pool}
              onChange={(e) => setPool(e.target.value as Pool)}
              className="rounded-tag border border-pebble bg-pure-card px-3 py-2 font-inter text-[14px] text-sepia focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sepia/40"
            >
              <option value="pass">Pass pool</option>
              <option value="fail">Fail pool</option>
            </select>
          </label>
          <label className="flex flex-col gap-1.5">
            <span className="font-inter text-[13px] font-medium text-sepia">Side</span>
            <select
              aria-label="Side"
              value={side}
              onChange={(e) => setSide(e.target.value as Side)}
              className="rounded-tag border border-pebble bg-pure-card px-3 py-2 font-inter text-[14px] text-sepia focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sepia/40"
            >
              <option value="buy">Buy (USDC → KASS)</option>
              <option value="sell">Sell (KASS → USDC)</option>
            </select>
          </label>
        </div>

        <Field
          label={`Amount in — ${inLabel}`}
          hint={`In ${inLabel}, e.g. 1.5.`}
          error={amountRaw !== '' ? parsed.error : undefined}
        >
          {(ids) => (
            <TextInput
              ids={ids}
              inputMode="decimal"
              placeholder="e.g. 1.5"
              value={amountRaw}
              onChange={(e) => setAmountRaw(e.target.value)}
            />
          )}
        </Field>

        <Field label="Slippage %" error={slip.error}>
          {(ids) => (
            <TextInput
              ids={ids}
              inputMode="decimal"
              placeholder="0.5"
              value={slipRaw}
              onChange={(e) => setSlipRaw(e.target.value)}
            />
          )}
        </Field>

        {/* Expected-out + price-impact preview (constant-product, CU1 reserves). */}
        <div className="rounded-tag border border-pebble bg-pure-card px-3 py-2 font-inter text-[12px]">
          {amm === null ? (
            <p className="text-driftwood">Pool not readable — no estimate.</p>
          ) : parsed.value === undefined ? (
            <p className="text-driftwood">Enter an amount to preview the expected output.</p>
          ) : (
            <dl className="flex flex-col gap-1">
              <div className="flex items-baseline justify-between gap-3">
                <dt className="text-driftwood">Expected out — {outLabel}</dt>
                <dd className="tabular-nums text-sepia">
                  ≈ {formatUnits(est.expectedOut, outDecimals)}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-3">
                <dt className="text-driftwood">Price impact</dt>
                <dd
                  className={`tabular-nums ${est.impact >= 0.1 ? 'text-ember-orange' : 'text-sepia'}`}
                >
                  ≈ {impactPct}%
                </dd>
              </div>
            </dl>
          )}
        </div>

        <div className="flex items-center gap-3">
          <SubmitButton
            verb="Swap"
            status={action.status}
            disabled={parsed.value === undefined || Boolean(slip.error)}
          />
        </div>
        <WriteStatusRegion status={action.status} successVerb="Swapped" />
      </form>
    </ConnectGate>
  )
}
