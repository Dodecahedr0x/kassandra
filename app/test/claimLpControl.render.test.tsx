/**
 * Regression for the LP claim panel copy: a post-activation liquidity provider
 * holds their position in `lateLp` with `amount == 0`, so the panel must describe
 * the LP they added — not read as a "0 KASS contribution". A pure funder still
 * reads as their KASS stake.
 */
import { vi } from "vitest";

const state = vi.hoisted(() => ({ address: "Late1111" as string | null }));

vi.mock("../src/market/hooks/useWriteAction", () => ({
  useWriteAction: () => ({
    status: { kind: "idle" },
    address: state.address,
    connected: true,
    indexer: {},
    run: async () => {},
  }),
}));

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { ClaimLpControl } from "../src/components/markets/actions/ClaimLpControl";

function contribution(contributor: string, amount: bigint, lateLp: bigint) {
  return { pubkey: `c-${contributor}`, contribution: { contributor: { toString: () => contributor }, amount, lateLp, claimed: false } };
}

function market(feeCollected: boolean) {
  return { feeCollected, lpMint: { toString: () => "LpMint1111" } } as never;
}

function render(feeCollected: boolean, contributions: unknown[]): string {
  return renderToStaticMarkup(
    <ClaimLpControl
      pubkey="Market1111"
      market={market(feeCollected)}
      contributions={contributions as never}
      onSuccess={() => {}}
    />,
  );
}

describe("ClaimLpControl position copy", () => {
  it("describes the added LP for a pure late LP (not 0 KASS)", () => {
    state.address = "Late1111";
    const html = render(false, [contribution("Late1111", 0n, 500_000_000n)]);
    expect(html).toContain("0.5 LP you added to the pool");
    expect(html).not.toContain("0 KASS");
  });

  it("describes the KASS stake for a pure funder", () => {
    state.address = "Fund1111";
    const html = render(true, [contribution("Fund1111", 1_000_000_000n, 0n)]);
    expect(html).toContain("1 KASS funding contribution");
    expect(html).toContain("Claim LP");
  });

  it("describes both for a funder who also added liquidity", () => {
    state.address = "Both1111";
    const html = render(false, [contribution("Both1111", 2_000_000_000n, 3_000_000_000n)]);
    expect(html).toContain("2 KASS funding contribution and 3 LP you added to the pool");
  });
});
