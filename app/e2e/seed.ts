/**
 * Reusable on-chain seeding for the browser E2E — drives oracles into each phase
 * with REAL instructions over surfpool (mirroring the gated surfpool vitest E2Es
 * in `app/test/*.e2e.test.ts`), so each Playwright spec can perform ONE app UI
 * action against an oracle already in the right phase.
 *
 * This is a thin barrel: the implementation lives in two sibling modules —
 * `seed-core.ts` (context, boot/init, tx sending, account fetch/fund, oracle
 * creation, and the account-fabrication cheatcodes) and `seed-drivers.ts` (the
 * phase drivers). Importers keep using `./seed.ts` / `../seed.ts`.
 *
 * IMPORTANT: every pubkey handed to an `@kassandra-market/oracles` builder is passed as a
 * base58 STRING (`.toString()`), never a web3.js `Address` object — under
 * Playwright's loader the app and the SDK resolve separate copies of
 * `@solana/web3.js`, so a foreign `Address` fails the SDK's `instanceof` check.
 */
export * from './seed-core.ts'
export * from './seed-drivers.ts'
