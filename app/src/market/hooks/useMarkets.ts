/**
 * Query hook over the read layer: every market from the indexer. Re-runs on mount
 * (and whenever the {@link IndexerClient} identity changes), plus a manual
 * `refetch` for the error-state retry.
 */
import { useIndexer } from "../lib/indexer";
import { fetchMarkets, type MarketSummary } from "../data/markets";
import { useAsync, useRefetchAfterWrite, type AsyncState } from "./useAsync";

export interface MarketsState extends AsyncState<MarketSummary[]> {
  /** Refetch resilient to indexer/RPC propagation lag — use as a write action's onSuccess. */
  refetchAfterWrite: () => void;
}

/** The market list: every mapped {@link MarketSummary}, most-funded first. */
export function useMarkets(): MarketsState {
  const indexer = useIndexer();
  const state = useAsync(() => fetchMarkets(indexer), [indexer]);
  const refetchAfterWrite = useRefetchAfterWrite(state.refetch);
  return { ...state, refetchAfterWrite };
}

export default useMarkets;
