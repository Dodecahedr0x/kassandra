import type { ActivateStep } from "../../../../market/data/actions";
import type { StepStatus } from "../../../../market/hooks/useActionSequence";

/** Compact per-outcome progress list for the batch sequence. */
export function BatchStepList({ steps, statuses }: { steps: ActivateStep[]; statuses: StepStatus[] }) {
  return (
    <ol aria-live="polite" className="flex flex-col gap-1.5">
      {steps.map((step, i) => {
        const st: StepStatus = statuses[i] ?? { kind: "pending" };
        const glyph =
          st.kind === "done" ? "✓" : st.kind === "error" ? "✕" : st.kind === "running" ? "…" : "○";
        const tone =
          st.kind === "done"
            ? "text-chestnut"
            : st.kind === "error"
              ? "text-ember-orange"
              : st.kind === "running"
                ? "text-bronze"
                : "text-stone";
        return (
          <li key={step.label} className="flex flex-col gap-0.5">
            <div className="flex items-center gap-2 font-inter text-[13px]">
              <span className={`w-3 text-center font-mono text-[12px] ${tone}`}>{glyph}</span>
              <span className={st.kind === "done" ? "text-chestnut" : "text-sepia"}>
                {i + 1}. {step.label}
              </span>
              {st.kind === "done" && st.signature === "already-landed" ? (
                <span className="font-mono text-[11px] text-stone">already on-chain</span>
              ) : null}
            </div>
            {st.kind === "error" ? (
              <p className="pl-6 font-inter text-[12px] text-ember-orange">{st.message}</p>
            ) : null}
          </li>
        );
      })}
    </ol>
  );
}
