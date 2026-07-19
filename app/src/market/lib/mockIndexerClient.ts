/**
 * The mock-mode counterpart to {@link IndexerClient} — same read surface, backed
 * by fixture DTOs instead of `fetch`. Swapped in by {@link IndexerProvider} under
 * `?mock` / `VITE_MOCK=1`. The write path is intentionally NOT implemented (each
 * method throws), matching how mock oracles never let you submit a real
 * transaction — there's no chain to send one to.
 */
import type {
  AccountRead,
  CandleDto,
  ConfigDto,
  IndexerReads,
  MarketDetailDto,
  MarketDto,
  SignatureStatus,
} from "./indexer";
import { mockCandles, mockConfig, mockMarketDetail, mockMarkets } from "../data/mockMarkets";

export class MockIndexerClient implements IndexerReads {
  /** Mirrors `IndexerClient.getConfig` — the fixture `Config` singleton. */
  async getConfig(): Promise<ConfigDto | null> {
    return mockConfig();
  }

  /** Mirrors `IndexerClient.getMarkets` — every fixture market. */
  async getMarkets(): Promise<MarketDto[]> {
    return mockMarkets();
  }

  /** Mirrors `IndexerClient.getMarket` — `null` for an unknown pubkey (mirrors a 404). */
  async getMarket(pubkey: string): Promise<MarketDetailDto | null> {
    return mockMarketDetail(pubkey);
  }

  /**
   * Mirrors `IndexerClient.getCandles` — a deterministic synthetic OHLC series.
   * Anchored to real wall-clock "now" (this is the live runtime bridge, not a
   * snapshotted script) so the series lines up with `PriceChart`'s wall-clock
   * visible window instead of sitting at a fixed historical epoch.
   */
  async getCandles(pubkey: string, intervalSecs: number, limit = 300): Promise<CandleDto[]> {
    return mockCandles(pubkey, intervalSecs, limit, Math.floor(Date.now() / 1000));
  }

  /** No raw account reads in mock mode — always resolves to `null`. */
  async getAccount(_pubkey: string): Promise<AccountRead | null> {
    return null;
  }

  /** Mock mode has no chain to fetch a blockhash from. */
  async getBlockhash(): Promise<string> {
    throw new Error("MockIndexerClient: writes are not supported in mock mode");
  }

  /** Mock mode has no chain to relay a transaction to. */
  async sendTransaction(_txBase64: string): Promise<string> {
    throw new Error("MockIndexerClient: writes are not supported in mock mode");
  }

  /** Mock mode never submits a transaction, so there is no status to poll. */
  async getSignatureStatus(_signature: string): Promise<SignatureStatus> {
    throw new Error("MockIndexerClient: writes are not supported in mock mode");
  }
}
