/**
 * Headless render coverage for the group bulk-liquidity panel. The data hooks are
 * mocked so the panel sees a categorical GROUP (3 Funding sub-markets on one
 * oracle) or a lone market, and we assert (via `renderToStaticMarkup`):
 *   - a group renders the "Group liquidity" panel + the uniform-split deposit UI;
 *   - a lone market renders NOTHING (self-hides — it uses the single-market form);
 *   - the deposit affordance targets exactly the funding outcomes.
 */
import { vi } from "vitest";

// Mutable data the mocked useMarkets returns (set per test).
const state = vi.hoisted(() => ({ siblings: [] as unknown[] }));

vi.mock("../src/market/hooks/useMarkets", () => ({
  useMarkets: () => ({ data: state.siblings, loading: false, error: undefined, refetch: () => {} }),
}));
vi.mock("../src/market/hooks/useMarketDetail", () => ({
  useConfig: () => ({ data: { kassMint: { toString: () => "Kass1111111111111111111111111111111111111111" } }, loading: false, error: undefined, refetch: () => {} }),
}));
vi.mock("../src/market/hooks/useKassBalance", () => ({
  useKassBalance: () => ({ balance: 1_000_000_000_000n, loading: false, refetch: () => {} }),
}));
vi.mock("../src/market/hooks/useActionSequence", () => ({
  useActionSequence: () => ({ statuses: [], busy: false, connected: true, address: "Wa11et11111111111111111111111111111111111111", allDone: false, run: async () => {}, reset: () => {} }),
}));
vi.mock("../src/market/lib/indexer", () => ({ useIndexer: () => ({}) }));
vi.mock("../src/components/markets/actions/ConnectGate", () => ({
  // Render children directly (connected) so the panel body is inspectable.
  ConnectGate: ({ children }: { children: unknown }) => children,
}));

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MarketStatus } from "@kassandra-market/markets";
import { describe, expect, it } from "vitest";

import { GroupLiquidityPanel } from "../src/components/markets/actions/GroupLiquidityPanel";

const ORACLE = "Orac1e1111111111111111111111111111111111111";

function summary(
  outcomeIndex: number,
  status: MarketStatus,
  feeCollected = false,
  reserves: { base: bigint; quote: bigint } | null = null,
) {
  return {
    pubkey: `Market${outcomeIndex}1111111111111111111111111111111111`,
    market: {
      oracle: { toString: () => ORACLE },
      outcomeIndex,
      status,
      feeCollected,
      lpMint: { toString: () => `LpMint${outcomeIndex}111111111111111111111111111111111` },
    },
    reserves,
    oracleOptionsCount: 3,
  } as unknown;
}

function render(): string {
  return renderToStaticMarkup(<GroupLiquidityPanel oracle={ORACLE} />);
}

describe("GroupLiquidityPanel", () => {
  it("renders the bulk deposit panel for a categorical group in funding", () => {
    state.siblings = [
      summary(0, MarketStatus.Funding),
      summary(1, MarketStatus.Funding),
      summary(2, MarketStatus.Funding),
    ];
    const html = render();
    expect(html).toContain("Group liquidity");
    expect(html).toContain("all 3 outcomes");
    // Deposit affordance targets the 3 depositable outcomes with a uniform split.
    expect(html).toContain("Deposit into 3 outcomes");
    expect(html).toMatch(/Split uniformly across 3 outcomes/);
  });

  it("offers bulk add-liquidity for an ACTIVE group (with reserves), skipping reserveless outcomes", () => {
    const r = { base: 1_000_000n, quote: 1_000_000n };
    state.siblings = [
      summary(0, MarketStatus.Active, false, r),
      summary(1, MarketStatus.Active, false, r),
      summary(2, MarketStatus.Active, false, null), // reserves not loaded → excluded
    ];
    const html = render();
    // Only the 2 outcomes with known reserves are depositable.
    expect(html).toContain("Deposit into 2 outcomes");
    expect(html).toMatch(/Split uniformly across 2 outcomes/);
  });

  it("closes deposits when no outcome can take liquidity", () => {
    state.siblings = [
      summary(0, MarketStatus.Active, false, null), // Active but reserveless
      summary(1, MarketStatus.Resolved),
    ];
    const html = render();
    expect(html).toContain("deposits are closed for this group");
  });

  it("self-hides for a lone market (not a group)", () => {
    state.siblings = [summary(0, MarketStatus.Funding)];
    expect(render()).toBe("");
  });

  it("only counts funding outcomes as depositable", () => {
    state.siblings = [
      summary(0, MarketStatus.Funding),
      summary(1, MarketStatus.Active), // active → not depositable
      summary(2, MarketStatus.Funding),
    ];
    const html = render();
    expect(html).toContain("all 3 outcomes"); // still a 3-outcome group
    expect(html).toContain("Deposit into 2 outcomes"); // but only 2 accept funding
  });

  it("offers bulk withdraw when outcomes have collected fees", () => {
    state.siblings = [
      summary(0, MarketStatus.Resolved, true),
      summary(1, MarketStatus.Resolved, true),
    ];
    const html = render();
    expect(html).toContain("Withdraw from 2 outcomes");
  });
});
