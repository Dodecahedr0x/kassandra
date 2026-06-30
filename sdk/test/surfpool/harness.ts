/**
 * `SurfpoolHarness` — drive a headless surfpool simnet from the (gated) E2E
 * suite (Task T1).
 *
 * Responsibilities:
 *   1. spawn `surfpool start --no-tui --block-production-mode transaction`
 *      (a standalone simnet; no `--network`/`--rpc-url` fork);
 *   2. poll the RPC `getHealth` until ready (with a timeout);
 *   3. deploy the LOCAL `target/deploy/kassandra_program.so` at the FIXED
 *      program id {@link KASSANDRA_PROGRAM_ID} via the `surfnet_setAccount`
 *      cheatcode (writing the ELF as a non-upgradeable BPFLoader2 program
 *      account — surfpool then JIT-loads + executes it, exactly like
 *      `solana-test-validator --bpf-program`);
 *   4. expose the RPC url + a web3.js {@link Connection};
 *   5. tear the child process down on completion.
 *
 * It also exposes small cheatcode helpers (`setAccount`, `airdrop`,
 * `timeTravelToSlot`) the smoke/lifecycle tests use to fabricate state.
 *
 * GATING: {@link surfpoolBinary} returns `null` when the `surfpool` binary is
 * not found, so the suite can SKIP (not fail) when surfpool is unavailable. The
 * default `pnpm test` never imports this file (see `vitest.config.ts`).
 */
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { Connection } from "@solana/web3.js";

import { KASSANDRA_PROGRAM_ID } from "../../src/constants.js";

const here = dirname(fileURLToPath(import.meta.url));
/** The local SBF artifact (`just build` produces it). */
export const SO_PATH = resolve(here, "../../../target/deploy/kassandra_program.so");

/** The deprecated (non-upgradeable) BPF loader: a program account IS its ELF. */
const BPF_LOADER_2 = "BPFLoader2111111111111111111111111111111111";

/** Candidate locations for the surfpool binary, in priority order. */
function surfpoolCandidates(): string[] {
  const fromEnv = process.env.SURFPOOL_BIN;
  return [
    ...(fromEnv ? [fromEnv] : []),
    join(homedir(), ".local/bin/surfpool"),
    "/usr/local/bin/surfpool",
    "/opt/homebrew/bin/surfpool",
  ];
}

/** Resolve the surfpool binary path, or `null` if it cannot be found. */
export function surfpoolBinary(): string | null {
  for (const c of surfpoolCandidates()) {
    if (existsSync(c)) return c;
  }
  return null;
}

/** True when both surfpool and the built `.so` are present (for `skipIf`). */
export function surfpoolReady(): boolean {
  return surfpoolBinary() !== null && existsSync(SO_PATH);
}

/** PATH augmented with the usual local solana/surfpool bin dirs. */
function augmentedPath(): string {
  const extra = [
    join(homedir(), ".local/bin"),
    join(homedir(), ".local/share/solana/install/active_release/bin"),
  ];
  return [...extra, process.env.PATH ?? ""].join(":");
}

export interface HarnessOptions {
  /** RPC port (default 8899). */
  port?: number;
  /** Readiness timeout in ms (default 30000). */
  readyTimeoutMs?: number;
}

export class SurfpoolHarness {
  private constructor(
    private readonly child: ChildProcess,
    readonly rpcUrl: string,
    readonly connection: Connection,
  ) {}

  /** Spawn surfpool, wait for readiness, and deploy the program. */
  static async start(opts: HarnessOptions = {}): Promise<SurfpoolHarness> {
    const bin = surfpoolBinary();
    if (!bin) throw new Error("surfpool binary not found (set SURFPOOL_BIN or install it)");
    if (!existsSync(SO_PATH)) {
      throw new Error(`Missing program artifact at ${SO_PATH}. Run \`just build\` first.`);
    }

    const port = opts.port ?? 8899;
    const rpcUrl = `http://127.0.0.1:${port}`;

    const child = spawn(
      bin,
      [
        "start",
        "--no-tui",
        "--block-production-mode",
        "transaction",
        "--no-deploy",
        "--port",
        String(port),
      ],
      {
        stdio: ["ignore", "ignore", "ignore"],
        env: { ...process.env, PATH: augmentedPath() },
        detached: false,
      },
    );

    const connection = new Connection(rpcUrl, "confirmed");
    const harness = new SurfpoolHarness(child, rpcUrl, connection);

    try {
      await harness.waitForHealth(opts.readyTimeoutMs ?? 30_000);
      await harness.deployProgram();
    } catch (e) {
      await harness.teardown();
      throw e;
    }
    return harness;
  }

  /** A raw JSON-RPC call (used for the `surfnet_*` cheatcodes). */
  async rpc<T = unknown>(method: string, params: unknown[]): Promise<T> {
    const res = await fetch(this.rpcUrl, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
    });
    const json = (await res.json()) as { result?: T; error?: { message: string } };
    if (json.error) throw new Error(`${method} failed: ${json.error.message}`);
    return json.result as T;
  }

  /** Poll `getHealth` until "ok" or the timeout elapses. */
  private async waitForHealth(timeoutMs: number): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    let lastErr = "";
    while (Date.now() < deadline) {
      if (this.child.exitCode !== null) {
        throw new Error(`surfpool exited early (code ${this.child.exitCode})`);
      }
      try {
        const health = await this.rpc<string>("getHealth", []);
        if (health === "ok") return;
      } catch (e) {
        lastErr = String(e);
      }
      await new Promise((r) => setTimeout(r, 250));
    }
    throw new Error(`surfpool did not become healthy within ${timeoutMs}ms (${lastErr})`);
  }

  /**
   * Write the local ELF at the fixed program id as a non-upgradeable BPFLoader2
   * program account. surfpool's `surfnet_setAccount` takes the account `data` as
   * a HEX string.
   */
  private async deployProgram(): Promise<void> {
    const elfHex = readFileSync(SO_PATH).toString("hex");
    await this.setAccount(KASSANDRA_PROGRAM_ID.toString(), {
      lamports: 5_000_000_000,
      owner: BPF_LOADER_2,
      executable: true,
      data: elfHex,
    });
  }

  /** `surfnet_setAccount` cheatcode: write/overwrite an account at `pubkey`. */
  async setAccount(
    pubkey: string,
    update: { lamports?: number; owner?: string; executable?: boolean; data?: string },
  ): Promise<void> {
    await this.rpc("surfnet_setAccount", [pubkey, update]);
  }

  /** Airdrop `lamports` to `pubkey` and wait until the balance reflects it. */
  async airdrop(pubkey: string, lamports: number): Promise<void> {
    await this.rpc("requestAirdrop", [pubkey, lamports]);
    const deadline = Date.now() + 10_000;
    while (Date.now() < deadline) {
      const bal = await this.rpc<{ value: number }>("getBalance", [pubkey]);
      if (bal.value >= lamports) return;
      await new Promise((r) => setTimeout(r, 200));
    }
    throw new Error(`airdrop to ${pubkey} did not settle`);
  }

  /** `surfnet_timeTravel` cheatcode: jump the clock to `absoluteSlot` (for T3). */
  async timeTravelToSlot(absoluteSlot: number): Promise<void> {
    await this.rpc("surfnet_timeTravel", [{ absoluteSlot }]);
  }

  /** Kill the surfpool child process. */
  async teardown(): Promise<void> {
    if (this.child.exitCode !== null) return;
    await new Promise<void>((resolveDone) => {
      this.child.once("exit", () => resolveDone());
      this.child.kill("SIGKILL");
      // Safety net if `exit` never fires.
      setTimeout(() => resolveDone(), 2_000);
    });
  }
}

// ---------------------------------------------------------------------------
// SPL layout fabrication (mirrors `test/e2e.test.ts`): minimal canonical Mint /
// token-Account byte layouts, written token-program-owned via `setAccount`.
// ---------------------------------------------------------------------------

const MINT_LEN = 82;

/** Pack an 82-byte SPL `Mint` (COption authority tag 1 = Some). */
export function mintBytes(authority: Uint8Array, supply: bigint, decimals: number): Uint8Array {
  const data = new Uint8Array(MINT_LEN);
  const dv = new DataView(data.buffer);
  dv.setUint32(0, 1, true); // mint_authority COption tag = Some
  data.set(authority, 4);
  dv.setBigUint64(36, supply, true);
  data[44] = decimals;
  data[45] = 1; // is_initialized
  return data;
}

/** Hex-encode a byte array for the `surfnet_setAccount` `data` field. */
export function toHex(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString("hex");
}
