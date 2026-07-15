import { type FormEvent } from 'react'
import type { Market, Oracle } from '@kassandra-market/oracles'
import { buildSettleFromMarketIxs } from '../../../../data/actions/challengeSettle'
import { useWriteAction } from '../../../../hooks/useWriteAction'
import { recallNonce } from '../../../../lib/nonceStore'
import { resolveOracleNonce } from '../../../../data/actions/finalize'
import { ConnectGate } from '../ConnectGate'
import { SubmitButton } from '../formPrimitives'
import { WriteStatusRegion } from '../WriteStatusRegion'

/** Recall the oracle's create nonce, else recover it via the pure PDA scan (RF1). */
function oracleNonce(oracle: string): Promise<bigint> {
  const recalled = recallNonce(oracle)
  return recalled !== null ? Promise.resolve(recalled) : resolveOracleNonce(oracle)
}

/**
 * The ONE-CLICK settle sub-form (no JSON paste). The full 21-account settle set
 * is DERIVED client-side from the decoded {@link Market} + {@link Oracle} (+ the
 * challenged proposer's authority, read off the fetched proposers) via
 * {@link buildSettleFromMarketIxs}; the connected wallet just presses "Settle".
 * The oracle nonce is recalled (or PDA-scanned) exactly like every other write.
 */
export function SettleButton({
  oracleKey,
  oracle,
  market,
  proposerAuthority,
  refetch,
}: {
  oracleKey: string
  oracle: Oracle
  market: Market
  /** The challenged proposer's wallet authority (owner of proposerUsdc). */
  proposerAuthority: string | undefined
  refetch: () => void
}) {
  const action = useWriteAction(refetch)

  const onSubmit = (e: FormEvent) => {
    e.preventDefault()
    void action.run(async () => {
      const nonce = await oracleNonce(oracleKey)
      return buildSettleFromMarketIxs({
        connection: action.connection,
        oracleNonce: nonce,
        market,
        oracle,
        proposerAuthority: proposerAuthority!,
        payer: action.address ?? undefined,
      })
    })
  }

  return (
    <ConnectGate connected={action.connected}>
      <form className="flex flex-col gap-3" onSubmit={onSubmit} noValidate>
        <div className="flex items-center gap-3">
          <SubmitButton
            verb="Settle challenge"
            status={action.status}
            disabled={proposerAuthority === undefined}
          />
        </div>
        {proposerAuthority === undefined ? (
          <p className="font-inter text-[12px] text-ember-orange">
            The challenged proposer isn&apos;t loaded yet — reload the oracle to settle.
          </p>
        ) : null}
        <WriteStatusRegion status={action.status} successVerb="Settled" />
      </form>
    </ConnectGate>
  )
}
