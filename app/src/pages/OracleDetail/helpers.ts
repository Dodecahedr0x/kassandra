import type { Address } from '@solana/web3.js'
import { recallNonce } from '../../lib/nonceStore'
import { resolveOracleNonce } from '../../data/actions/finalize'

/**
 * Settlement context threaded to the claim / close controls once an oracle is
 * terminal (Resolved / InvalidDeadend). Present ⇒ render the payout controls.
 */
export interface SettleCtx {
  oracle: string
  kassMint: Address
  refetch: () => void
}

/** Recall the oracle's create nonce, else recover it via the pure PDA scan (RF1). */
export function oracleNonce(oracle: string): Promise<bigint> {
  const recalled = recallNonce(oracle)
  return recalled !== null ? Promise.resolve(recalled) : resolveOracleNonce(oracle)
}
