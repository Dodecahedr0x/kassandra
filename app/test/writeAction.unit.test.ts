/**
 * WF2 offline unit tests for the write-action state machine (default suite — no
 * network, no React). Drives {@link runWriteAction} with a mock sender + a mock
 * Connection (whose `getSignatureStatuses` confirms or fails on demand) and
 * asserts the full status sequence + the error mapping (validation, user
 * rejection, a program-log send error, a failed confirm).
 */
import type { Connection, TransactionInstruction } from '@solana/web3.js'
import { describe, expect, it } from 'vitest'
import { ValidationError } from '../src/data/actions.ts'
import { SendError, type TxSender } from '../src/data/send.ts'
import {
  isUserRejection,
  mapWriteError,
  runWriteAction,
  type WriteStatus,
} from '../src/data/writeAction.ts'

/** A mock Connection whose signature status is confirmed (or errored). */
function mockConnection(opts: { confirmErr?: unknown } = {}): Connection {
  return {
    getSignatureStatuses: async () => ({
      value: [{ err: opts.confirmErr ?? null, confirmationStatus: 'confirmed' }],
    }),
  } as unknown as Connection
}

const noIxs = async (): Promise<TransactionInstruction[]> => []
const kinds = (s: WriteStatus[]) => s.map((x) => x.kind)

describe('runWriteAction — happy path', () => {
  it('drives building → signing → confirming → success and calls onSuccess', async () => {
    const seen: WriteStatus[] = []
    let successSig: string | undefined
    const sender: TxSender = async () => 'SIG_OK'
    const final = await runWriteAction({
      build: noIxs,
      connection: mockConnection(),
      walletSender: sender,
      setStatus: (s) => seen.push(s),
      onSuccess: (sig) => {
        successSig = sig
      },
    })
    expect(kinds(seen)).toEqual(['building', 'signing', 'confirming', 'success'])
    expect(final).toEqual({ kind: 'success', signature: 'SIG_OK' })
    expect(successSig).toBe('SIG_OK')
  })
})

describe('runWriteAction — error mapping', () => {
  it('surfaces a build ValidationError before signing', async () => {
    const seen: WriteStatus[] = []
    const final = await runWriteAction({
      build: async () => {
        throw new ValidationError('bond', 'bond must be greater than zero.')
      },
      connection: mockConnection(),
      walletSender: async () => 'unused',
      setStatus: (s) => seen.push(s),
    })
    expect(kinds(seen)).toEqual(['building', 'error'])
    expect(final).toEqual({ kind: 'error', message: 'bond must be greater than zero.' })
  })

  it('maps a wallet user-rejection (code 4001) to a friendly message', async () => {
    const seen: WriteStatus[] = []
    const reject: TxSender = async () => {
      const e = Object.assign(new Error('User rejected the request.'), { code: 4001 })
      throw e
    }
    const final = await runWriteAction({
      build: noIxs,
      connection: mockConnection(),
      walletSender: reject,
      setStatus: (s) => seen.push(s),
    })
    expect(kinds(seen)).toEqual(['building', 'signing', 'error'])
    expect(final).toEqual({ kind: 'error', message: 'Transaction rejected in wallet.' })
  })

  it('keeps the send error message + program logs', async () => {
    const seen: WriteStatus[] = []
    const logs = ['Program log: Error: WrongPhase', 'custom program error: 0x1']
    const fail: TxSender = async () => {
      throw Object.assign(new Error('Simulation failed'), { logs })
    }
    const final = await runWriteAction({
      build: noIxs,
      connection: mockConnection(),
      walletSender: fail,
      setStatus: (s) => seen.push(s),
    })
    expect(final.kind).toBe('error')
    if (final.kind === 'error') {
      expect(final.message).toContain('Simulation failed')
      expect(final.logs).toEqual(logs)
    }
    // signing began but confirming never did
    expect(kinds(seen)).toEqual(['building', 'signing', 'error'])
  })

  it('surfaces a failed confirmation (tx err) after confirming', async () => {
    const seen: WriteStatus[] = []
    const final = await runWriteAction({
      build: noIxs,
      connection: mockConnection({ confirmErr: { InstructionError: [0, { Custom: 1 }] } }),
      walletSender: async () => 'SIG_BAD',
      setStatus: (s) => seen.push(s),
    })
    expect(kinds(seen)).toEqual(['building', 'signing', 'confirming', 'error'])
    expect(final.kind).toBe('error')
    if (final.kind === 'error') expect(final.message).toContain('failed to confirm')
  })
})

describe('mapWriteError / isUserRejection', () => {
  it('detects rejection by code, message, and wrapping SendError', () => {
    expect(isUserRejection(Object.assign(new Error('x'), { code: 4001 }))).toBe(true)
    expect(isUserRejection(new Error('User rejected the request.'))).toBe(true)
    expect(isUserRejection(new Error('Transaction cancelled'))).toBe(true)
    expect(
      isUserRejection(new SendError('send failed', { cause: Object.assign(new Error('nope'), { code: 4001 }) })),
    ).toBe(true)
    expect(isUserRejection(new Error('blockhash expired'))).toBe(false)
  })

  it('maps a ValidationError to its field message', () => {
    expect(mapWriteError(new ValidationError('uri', 'uri is 201 bytes (max 200).'))).toEqual({
      message: 'uri is 201 bytes (max 200).',
    })
  })

  it('passes a SendError message + logs through', () => {
    const err = new SendError('Transaction send failed: boom', { logs: ['a', 'b'] })
    expect(mapWriteError(err)).toEqual({ message: 'Transaction send failed: boom', logs: ['a', 'b'] })
  })
})
