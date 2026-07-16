import { useMemo, useState, type FormEvent } from "react";
import { MarketStatus } from "@kassandra-market/markets";
import { Button, Card } from "../../ui";
import { useMarkets } from "../../../market/hooks/useMarkets";
import { useConfig } from "../../../market/hooks/useMarketDetail";
import { useKassBalance } from "../../../market/hooks/useKassBalance";
import { useActionSequence } from "../../../market/hooks/useActionSequence";
import { useIndexer } from "../../../market/lib/indexer";
import {
  buildBulkClaimLpSteps,
  buildBulkContributeSteps,
  uniformSplit,
  type ActivateStep,
} from "../../../market/data/actions";
import { parseKassAmount, balanceGateError } from "../../../market/data/amount";
import { formatKass, outcomeLabel } from "../../../market/lib/marketView";
import type { MarketSummary } from "../../../market/data/markets";
import { ConnectGate } from "./ConnectGate";
import { Field, KassBalanceLine, TextInput } from "./formPrimitives";
import { BatchStepList } from "./CreateMarketForm/BatchStepList";

/**
 * Bulk liquidity for a categorical oracle's GROUP of sub-markets: deposit into,
 * or withdraw LP from, several/all outcomes at once. The default deposit splits
 * the entered total UNIFORMLY across every outcome still in funding
 * ({@link uniformSplit}); withdraw claims LP across every outcome whose fee has
 * been collected. Both fan the single-market builders into one
 * {@link useActionSequence} run. Renders nothing for a lone market (not a group).
 */
export function GroupLiquidityPanel({ oracle }: { oracle: string }) {
  const indexer = useIndexer();
  const config = useConfig();
  const kassMint = config.data ? config.data.kassMint.toString() : undefined;
  const { balance, loading: balanceLoading, refetch: refetchBalance } = useKassBalance(kassMint);
  const { data: allMarkets } = useMarkets();

  // The group = every sub-market on this oracle, in outcome order.
  const siblings = useMemo<MarketSummary[]>(
    () =>
      (allMarkets ?? [])
        .filter((m) => m.market.oracle.toString() === oracle)
        .sort((a, b) => a.market.outcomeIndex - b.market.outcomeIndex),
    [allMarkets, oracle],
  );

  const funding = useMemo(
    () => siblings.filter((m) => m.market.status === MarketStatus.Funding),
    [siblings],
  );
  const claimable = useMemo(
    () => siblings.filter((m) => m.market.feeCollected),
    [siblings],
  );

  const [total, setTotal] = useState("");
  const [error, setError] = useState<string | undefined>();
  const [steps, setSteps] = useState<ActivateStep[]>([]);

  const seq = useActionSequence(() => {
    refetchBalance();
  });

  // Parse the total + its uniform per-outcome split across the funding markets.
  const parsed = total.trim() === "" ? null : parseKassAmount(total);
  const totalValue = parsed?.value ?? null;
  const shares = totalValue !== null ? uniformSplit(totalValue, funding.length) : [];
  const perShareLabel =
    funding.length > 0 && totalValue !== null && totalValue > 0n
      ? `${formatKass(shares[0])}${shares.some((s) => s !== shares[0]) ? "–" + formatKass(shares.find((s) => s !== shares[0])!) : ""} KASS each`
      : null;

  async function onDeposit(e: FormEvent) {
    e.preventDefault();
    setError(undefined);
    if (!kassMint || !seq.address) return;
    if (parsed?.error) return setError(parsed.error);
    if (totalValue === null || totalValue <= 0n) return setError("Enter an amount to deposit.");
    const gate = balanceGateError(totalValue, balance);
    if (gate) return setError(gate);

    const entries = funding.map((m, i) => ({
      market: m.pubkey,
      label: outcomeLabel(m.market.outcomeIndex),
      amount: shares[i],
    }));
    try {
      const built = await buildBulkContributeSteps({
        indexer,
        kassMint,
        contributor: seq.address,
        entries,
      });
      seq.reset();
      setSteps(built);
      await seq.run(built);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onWithdraw() {
    setError(undefined);
    if (!seq.address) return;
    const entries = claimable.map((m) => ({
      market: m.pubkey,
      label: outcomeLabel(m.market.outcomeIndex),
      lpMint: m.market.lpMint.toString(),
    }));
    try {
      const built = await buildBulkClaimLpSteps({ indexer, contributor: seq.address, entries });
      seq.reset();
      setSteps(built);
      await seq.run(built);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  // Not a group (a lone market uses its own Contribute / Claim-LP controls).
  if (siblings.length <= 1) return null;

  return (
    <Card className="flex flex-col gap-4">
      <div>
        <h3 className="font-serif text-subheading font-light text-sepia">Group liquidity</h3>
        <p className="mt-1 font-inter text-[13px] text-bronze">
          Fund or withdraw across all {siblings.length} outcomes of this market at once.
        </p>
      </div>

      <ConnectGate connected={seq.connected}>
        <div className="flex flex-col gap-5">
          {/* Deposit — uniform split across the outcomes still in funding. */}
          {funding.length > 0 ? (
            <form onSubmit={onDeposit} className="flex flex-col gap-2">
              <Field
                label="Deposit (total KASS)"
                hint={
                  perShareLabel
                    ? `Split uniformly across ${funding.length} funding outcome${funding.length > 1 ? "s" : ""} · ${perShareLabel}`
                    : `Split uniformly across ${funding.length} funding outcome${funding.length > 1 ? "s" : ""}`
                }
                error={error}
              >
                {(ids) => (
                  <TextInput
                    ids={ids}
                    inputMode="decimal"
                    placeholder="0.0"
                    value={total}
                    onChange={(ev) => setTotal(ev.target.value)}
                  />
                )}
              </Field>
              <KassBalanceLine balance={balance} loading={balanceLoading} format={formatKass} />
              <div>
                <Button type="submit" variant="PrimaryChestnut" disabled={seq.busy}>
                  {seq.busy ? "Depositing…" : `Deposit into ${funding.length} outcomes`}
                </Button>
              </div>
            </form>
          ) : (
            <p className="font-inter text-[13px] text-driftwood">
              No outcomes are in funding — deposits are closed for this group.
            </p>
          )}

          {/* Withdraw — claim LP across every outcome whose fee has been collected. */}
          {claimable.length > 0 ? (
            <div className="flex flex-col gap-2 border-t border-pebble pt-4">
              <p className="font-inter text-[13px] text-bronze">
                Withdraw your LP from {claimable.length} settled outcome
                {claimable.length > 1 ? "s" : ""}.
              </p>
              <div>
                <Button type="button" variant="GhostOutline" disabled={seq.busy} onClick={onWithdraw}>
                  {seq.busy ? "Withdrawing…" : `Withdraw from ${claimable.length} outcomes`}
                </Button>
              </div>
            </div>
          ) : null}

          {steps.length > 0 ? <BatchStepList steps={steps} statuses={seq.statuses} /> : null}
        </div>
      </ConnectGate>
    </Card>
  );
}

export default GroupLiquidityPanel;
