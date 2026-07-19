import { describe, expect, it } from "vitest";
import { mockCandles, mockConfig, mockMarketDetail, mockMarkets, MOCK_MARKET_PUBKEYS } from "../src/market/data/mockMarkets";
import { groupByOracle, mapConfigDto, mapMarketDto } from "../src/market/data/markets";

describe("mockMarkets fixtures", () => {
  it("has at least 5 fixture pubkeys", () => {
    expect(MOCK_MARKET_PUBKEYS.length).toBeGreaterThanOrEqual(5);
  });

  it("every DTO round-trips through mapMarketDto (real Address + BigInt parsing)", async () => {
    const dtos = await mockMarkets();
    expect(dtos.length).toBeGreaterThanOrEqual(5);
    for (const dto of dtos) {
      // Throws if any pubkey isn't a genuinely valid base58 32-byte address, or
      // any u64 string isn't BigInt-parseable — exactly what the live fetch path does.
      expect(() => mapMarketDto(dto)).not.toThrow();
    }
  });

  it("covers at least three distinct market lifecycle statuses", async () => {
    const dtos = await mockMarkets();
    const mapped = dtos.map((d) => mapMarketDto(d));
    expect(new Set(mapped.map((m) => m.status)).size).toBeGreaterThanOrEqual(3);
  });

  it("includes one categorical group (>2 sub-markets sharing an oracle)", async () => {
    const dtos = await mockMarkets();
    const summaries = dtos.map((d) => ({
      pubkey: d.address,
      market: mapMarketDto(d),
      reserves: null,
      oracleOptionsCount: null,
    }));
    const groups = groupByOracle(summaries);
    expect(groups.some((g) => g.markets.length > 2)).toBe(true);
  });

  it("includes at least one Active market with populated (non-zero) reserves", async () => {
    const dtos = await mockMarkets();
    let found = false;
    for (const dto of dtos) {
      const mapped = mapMarketDto(dto);
      if (mapped.status !== 1 /* Active */) continue;
      const detail = await mockMarketDetail(dto.address);
      if (detail?.reserves && (BigInt(detail.reserves.base) > 0n || BigInt(detail.reserves.quote) > 0n)) {
        found = true;
      }
    }
    expect(found).toBe(true);
  });

  it("mockMarketDetail returns non-empty contributions for every fixture pubkey, mapping cleanly", async () => {
    for (const pubkey of MOCK_MARKET_PUBKEYS) {
      const detail = await mockMarketDetail(pubkey);
      expect(detail).not.toBeNull();
      expect(detail!.contributions.length).toBeGreaterThan(0);
      // Every contribution DTO must also round-trip.
      for (const c of detail!.contributions) {
        expect(() => BigInt(c.amount)).not.toThrow();
        expect(() => BigInt(c.lateLp)).not.toThrow();
      }
    }
  });

  it("mockMarketDetail returns null for an unknown pubkey", async () => {
    const detail = await mockMarketDetail("not-a-real-pubkey");
    expect(detail).toBeNull();
  });

  it("mockCandles is deterministic for identical inputs", async () => {
    const a = await mockCandles(MOCK_MARKET_PUBKEYS[0], 3600, 50);
    const b = await mockCandles(MOCK_MARKET_PUBKEYS[0], 3600, 50);
    expect(a).toEqual(b);
    expect(a.length).toBe(50);
    // Anchored to a fixed epoch, spaced by intervalSecs — never wall-clock time.
    expect(a[1].time - a[0].time).toBe(3600);
    for (const candle of a) {
      expect(candle.high).toBeGreaterThanOrEqual(Math.max(candle.open, candle.close));
      expect(candle.low).toBeLessThanOrEqual(Math.min(candle.open, candle.close));
    }
  });

  it("mockCandles anchors the last candle near a supplied nowSecs, keeping values deterministic", async () => {
    const intervalSecs = 3600;
    const nowSecs = Math.floor(Date.now() / 1000);
    const a = await mockCandles(MOCK_MARKET_PUBKEYS[0], intervalSecs, 50, nowSecs);
    const b = await mockCandles(MOCK_MARKET_PUBKEYS[0], intervalSecs, 50, nowSecs);
    // Same nowSecs -> same series (values still come from the seeded random walk).
    expect(a).toEqual(b);
    const last = a[a.length - 1];
    expect(last.time).toBeLessThanOrEqual(nowSecs);
    expect(nowSecs - last.time).toBeLessThan(intervalSecs);
    // The OHLC *values* match the fixed-epoch call (same seed, same walk) — only
    // the time axis moved.
    const fixed = await mockCandles(MOCK_MARKET_PUBKEYS[0], intervalSecs, 50);
    expect(a.map((c) => ({ open: c.open, high: c.high, low: c.low, close: c.close }))).toEqual(
      fixed.map((c) => ({ open: c.open, high: c.high, low: c.low, close: c.close })),
    );
  });

  it("mockConfig round-trips through mapConfigDto", async () => {
    const dto = await mockConfig();
    expect(() => mapConfigDto(dto)).not.toThrow();
  });
});
