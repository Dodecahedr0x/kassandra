/**
 * The Markets flow's offline-preview fixture set — the market analogue of
 * `src/data/mockOracles/index.ts`. Wired into a `MockIndexerClient` (a later
 * task) that intercepts `IndexerClient` calls in `?mock` / `VITE_MOCK=1` mode;
 * this module only exposes the fixture DATA as async functions shaped like the
 * real `IndexerClient` methods.
 */
import type { CandleDto, ConfigDto, MarketDetailDto, MarketDto } from "../../lib/indexer";
import { mockCandlesFor, mockConfigDto, mockMarketDetailFor, mockMarketDtos, MOCK_MARKET_PUBKEYS } from "./fixtures";

export { MOCK_MARKET_PUBKEYS };

/** Mock of `IndexerClient.getMarkets` — every fixture market. */
export async function mockMarkets(): Promise<MarketDto[]> {
  return mockMarketDtos();
}

/** Mock of `IndexerClient.getMarket` — `null` for an unknown pubkey (mirrors a 404). */
export async function mockMarketDetail(pubkey: string): Promise<MarketDetailDto | null> {
  return mockMarketDetailFor(pubkey);
}

/** Mock of `IndexerClient.getCandles` — a deterministic synthetic OHLC series. */
export async function mockCandles(pubkey: string, intervalSecs: number, limit: number, nowSecs?: number): Promise<CandleDto[]> {
  return mockCandlesFor(pubkey, intervalSecs, limit, nowSecs);
}

/** Mock of `IndexerClient.getConfig` — the fixture `Config` singleton. */
export async function mockConfig(): Promise<ConfigDto> {
  return mockConfigDto();
}
