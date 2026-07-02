import type { FormEvent } from 'react'
import type { TransactionInstruction } from '@solana/web3.js'
import { Card } from '../../ui'
import type { FinalizeAction } from '../../../data/actions/finalize'
import { useWriteAction } from '../../../hooks/useWriteAction'
import { ConnectGate } from './ConnectGate'
import { SubmitButton } from './formPrimitives'
import { WriteStatusRegion } from './WriteStatusRegion'

/**
 * A permissionless CRANK control (RF1): a chestnut button that runs one
 * finalize/advance action via the wallet-backed sender + the shared write-status
 * UX + a refetch on success. The finalize instructions carry no required signer,
 * so ANY connected wallet may crank — the wallet is only the fee-payer (noted in
 * the copy).
 *
 * `build` returns a {@link FinalizeAction}; this control sends the LEGACY path
 * (its `ixs`). When the oracle's proposer tail is large enough to overflow a
 * legacy transaction (`nearCap`, only the full-set finalize-proposals /
 * finalize-oracle cranks), the browser wallet cannot drive the multi-tx v0/ALT
 * setup, so instead of a dead button it renders a note pointing at the
 * keypair/CLI ALT path (`sendFinalizeSmart` with an `altKeypair`).
 */
export function FinalizeControl({
  title,
  description,
  verb,
  successVerb,
  build,
  refetch,
  nearCap = false,
}: {
  title: string
  description: string
  /** Idle button label, e.g. "Finalize proposals". */
  verb: string
  /** Past-tense confirmation verb, e.g. "Finalized". */
  successVerb: string
  /** Assemble the finalize action at click time (skipped under mock mode). */
  build: () => Promise<FinalizeAction>
  refetch: () => void
  /** True when the proposer tail overflows a legacy tx → ALT/CLI path required. */
  nearCap?: boolean
}) {
  const action = useWriteAction(refetch)

  const onSubmit = (e: FormEvent) => {
    e.preventDefault()
    void action.run(async (): Promise<TransactionInstruction[]> => {
      const fa = await build()
      return fa.ixs
    })
  }

  return (
    <Card className="flex flex-col gap-4">
      <div>
        <h3 className="font-serif text-subheading font-light text-sepia">{title}</h3>
        <p className="mt-1 font-inter text-[13px] text-driftwood">{description}</p>
        <p className="mt-1 font-inter text-[12px] text-driftwood">
          Permissionless — any connected wallet can crank this; it only pays the fee.
        </p>
      </div>
      {nearCap ? (
        <div className="rounded-tag border border-pebble bg-soft-cream px-3 py-2">
          <p className="font-inter text-[13px] text-bronze">
            This oracle's proposer set is too large to finalize in a single browser transaction.
            Run the finalize via the v0/ALT path (a keypair-driven crank) to advance it.
          </p>
        </div>
      ) : (
        <ConnectGate connected={action.connected}>
          <form className="flex flex-col gap-3" onSubmit={onSubmit} noValidate>
            <div className="flex items-center gap-3">
              <SubmitButton verb={verb} status={action.status} />
            </div>
            <WriteStatusRegion status={action.status} successVerb={successVerb} />
          </form>
        </ConnectGate>
      )}
    </Card>
  )
}

export default FinalizeControl
