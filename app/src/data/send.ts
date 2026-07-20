/**
 * WF1 — the tx SEND-AND-CONFIRM seam (pure, NO React).
 *
 * A {@link TxSender} abstracts "sign + submit a legacy tx built from these
 * instructions, return the signature". Both a wallet and a keypair satisfy it:
 *   - the UI (WF2) backs it with wallet-adapter's
 *     `sendTransaction(new Transaction().add(...ixs), connection)`;
 *   - tests back it with {@link keypairSender} (a funded {@link Keypair}).
 *
 * {@link sendAndConfirm} calls the sender then confirms the signature (reusing
 * the SDK's `confirmSignature` — a `getSignatureStatuses` poll), surfacing a
 * failed send / failed-to-confirm / expired tx as a typed {@link SendError}
 * (carrying the signature + any program logs) the caller can render.
 */
import { Connection, Keypair, Transaction, type TransactionInstruction } from "@solana/web3.js";
import { confirmSignature, decodeError, KASSANDRA_PROGRAM_ID } from "@kassandra-market/oracles";

/**
 * The signer-abstraction seam: given the instruction list, sign + submit a
 * legacy transaction and resolve to its base58 signature. The UI supplies a
 * wallet-backed sender; tests supply {@link keypairSender}.
 */
export type TxSender = (ixs: TransactionInstruction[]) => Promise<string>;

/** The successful outcome of {@link sendAndConfirm}. */
export interface SendResult {
  /** The confirmed transaction signature (base58). */
  signature: string;
}

/** A typed failure of the send/confirm path — carries the signature + program logs when known. */
export class SendError extends Error {
  /** The tx signature, if the send succeeded but confirmation failed. */
  readonly signature?: string;
  /** Program logs pulled off the underlying send/simulate error, if any. */
  readonly logs?: string[];
  /** The underlying error thrown by the sender / confirm. */
  readonly cause?: unknown;
  constructor(message: string, opts?: { signature?: string; logs?: string[]; cause?: unknown }) {
    super(message);
    this.name = "SendError";
    this.signature = opts?.signature;
    this.logs = opts?.logs;
    this.cause = opts?.cause;
  }
}

function errMsg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Best-effort extraction of program logs off a web3.js send/simulate error. */
function extractLogs(e: unknown): string[] | undefined {
  const logs = (e as { logs?: unknown } | null)?.logs;
  return Array.isArray(logs) ? (logs as string[]) : undefined;
}

/**
 * Turn a Kassandra `Custom(<code>)` program error into its human message (via
 * `decodeError`), or `undefined` when the failure isn't OUR program's error (a
 * MetaDAO / SPL / other-program `Custom` code won't decode — the caller falls
 * back to the raw text).
 *
 * To avoid mis-attributing another program's `Custom(N)` (small codes collide
 * across programs), this PREFERS a program-scoped log line — `Program
 * <KASSANDRA_PROGRAM_ID> failed: custom program error: 0xN` — and only falls
 * back to a bare `"Custom":N` from the status-error JSON (the confirm path,
 * where `getSignatureStatuses` gives no logs — see `confirmSignature`).
 */
export function humanizeProgramError(text: string, logs?: string[]): string | undefined {
  const program = KASSANDRA_PROGRAM_ID.toString();
  let code: number | undefined;

  const scoped = (logs ?? []).find(
    (l) => l.includes(program) && /custom program error:\s*0x[0-9a-fA-F]+/i.test(l),
  );
  if (scoped) {
    const h = /custom program error:\s*0x([0-9a-fA-F]+)/i.exec(scoped);
    if (h) code = parseInt(h[1], 16);
  }
  if (code === undefined) {
    const j = /"Custom"\s*:\s*(\d+)/.exec(text);
    if (j) code = Number(j[1]);
  }
  if (code === undefined) return undefined;

  const { name, message } = decodeError(code);
  return name === "Unknown" ? undefined : message;
}

/**
 * Send `ixs` via `sender`, then confirm the resulting signature over
 * `connection`. Throws a {@link SendError} (with the signature + logs when
 * available) if the send throws or the tx fails / never confirms — the
 * message is a decoded {@link humanizeProgramError} when the failure is a
 * recognized Kassandra custom error, else the raw error text.
 */
export async function sendAndConfirm(
  connection: Connection,
  sender: TxSender,
  ixs: TransactionInstruction[],
): Promise<SendResult> {
  let signature: string;
  try {
    signature = await sender(ixs);
  } catch (e) {
    const logs = extractLogs(e);
    const human = humanizeProgramError(errMsg(e), logs);
    throw new SendError(human ? `Transaction failed: ${human}` : `Transaction send failed: ${errMsg(e)}`, {
      logs,
      cause: e,
    });
  }
  try {
    await confirmSignature(connection, signature);
  } catch (e) {
    const logs = extractLogs(e);
    const human = humanizeProgramError(errMsg(e), logs);
    throw new SendError(
      human
        ? `Transaction ${signature} failed: ${human}`
        : `Transaction ${signature} failed to confirm: ${errMsg(e)}`,
      { signature, logs, cause: e },
    );
  }
  return { signature };
}

/**
 * A keypair-backed {@link TxSender} for tests/CLIs: builds a LEGACY
 * {@link Transaction} with a fresh blockhash, sets `keypair` as the fee payer,
 * signs with it, and `sendRawTransaction`s it (preflight on). Returns the
 * signature — {@link sendAndConfirm} then confirms it.
 *
 * NOTE: the UI does NOT use this — WF2 supplies its own wallet-adapter-backed
 * sender (`(ixs) => sendTransaction(new Transaction().add(...ixs), connection)`).
 */
export function keypairSender(connection: Connection, keypair: Keypair): TxSender {
  return async (ixs) => {
    const tx = new Transaction();
    tx.feePayer = keypair.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    for (const ix of ixs) tx.add(ix);
    await tx.sign(keypair);
    return connection.sendRawTransaction(await tx.serialize(), { skipPreflight: false });
  };
}
