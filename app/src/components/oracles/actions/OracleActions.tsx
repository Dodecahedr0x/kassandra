import { Phase, type Oracle } from '@kassandra/sdk'
import { Card } from '../../ui'
import { phaseView } from '../../../lib/oracleView'
import { ProposeForm } from './ProposeForm'
import { SubmitFactForm } from './SubmitFactForm'

/** Muted "participation closed / redirected" note for the non-form phases. */
function Note({ children }: { children: React.ReactNode }) {
  return (
    <Card>
      <p className="font-inter text-[14px] text-driftwood">{children}</p>
    </Card>
  )
}

/**
 * The "Participate" surface on the oracle detail page. Phase-gated: the propose
 * form in Proposal, the submit-fact form in FactProposal, a pointer to the
 * per-fact vote controls in FactVoting, and a muted closed-note otherwise. The
 * per-fact {@link VoteControl}s live on the fact cards themselves.
 */
export function OracleActions({
  pubkey,
  oracle,
  refetch,
}: {
  pubkey: string
  oracle: Oracle
  refetch: () => void
}) {
  const phaseLabel = phaseView(oracle.phase).label

  return (
    <section className="mt-14">
      <h2 className="font-serif text-heading-sm font-light text-sepia">Participate</h2>
      <div className="mt-4">
        {oracle.phase === Phase.Proposal ? (
          <ProposeForm pubkey={pubkey} oracle={oracle} refetch={refetch} />
        ) : oracle.phase === Phase.FactProposal ? (
          <SubmitFactForm pubkey={pubkey} oracle={oracle} refetch={refetch} />
        ) : oracle.phase === Phase.FactVoting ? (
          <Note>
            This oracle is in fact voting — approve or flag facts using the controls on each fact in
            the Facts section below.
          </Note>
        ) : (
          <Note>Participation is closed — this oracle is in the {phaseLabel} phase.</Note>
        )}
      </div>
    </section>
  )
}

export default OracleActions
