/**
 * Offline unit tests for `src/data/send.ts`'s custom-error decoding —
 * {@link humanizeProgramError} turns a Kassandra `Custom(<code>)` program
 * error (surfaced either as a scoped log line or the confirm-path status
 * JSON) into the human message from `KassandraError`/`decodeError`.
 */
import { KASSANDRA_PROGRAM_ID } from "@kassandra-market/oracles";
import { describe, expect, it } from "vitest";

import { humanizeProgramError } from "../src/data/send.ts";

describe("humanizeProgramError", () => {
  it("decodes a program-scoped 'custom program error: 0x..' log line", () => {
    const logs = [
      `Program ${KASSANDRA_PROGRAM_ID.toString()} invoke [1]`,
      "Program log: Error: WrongPhase",
      `Program ${KASSANDRA_PROGRAM_ID.toString()} failed: custom program error: 0x1`,
    ];
    expect(humanizeProgramError("Simulation failed", logs)).toBe(
      "The oracle is not in the phase this instruction requires.",
    );
  });

  it("falls back to a bare \"Custom\":N in the status-error JSON when there are no logs", () => {
    const text = `tx SIG failed: {"InstructionError":[0,{"Custom":4}]}`;
    // Custom(4) == KassandraError.Unauthorized.
    expect(humanizeProgramError(text)).toBe("The signer is not authorized to perform this action.");
  });

  it("returns undefined for an unrecognized code", () => {
    expect(humanizeProgramError(`{"Custom":9999}`)).toBeUndefined();
  });

  it("returns undefined when no custom-error shape is present", () => {
    expect(humanizeProgramError("blockhash not found")).toBeUndefined();
  });

  it("does not decode another program's Custom(N) scoped log line", () => {
    const otherProgram = "VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg";
    const logs = [`Program ${otherProgram} failed: custom program error: 0x1`];
    // No KASSANDRA_PROGRAM_ID-scoped line and no bare "Custom":N in the text → undefined.
    expect(humanizeProgramError("Simulation failed", logs)).toBeUndefined();
  });
});
