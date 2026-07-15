//! `open_challenge`: a challenger opens a MetaDAO decision market against a
//! proposer's [`AiClaim`] during the `Challenge` window.
//!
//! # Decomposed market (design §6)
//! The challenger composes the MetaDAO accounts in their OWN transactions
//! (like the Task 9 tests): a binary `question` whose resolver is the Kassandra
//! oracle PDA (outcome 0 = pass, 1 = fail), a KASS conditional vault, a USDC
//! conditional vault, and the pass/fail AMMs. This instruction does NOT create
//! them — it **verifies** they are bound to this oracle/claim, **records** them
//! in a [`Market`] PDA, performs the **program-signed** split of the proposer's
//! already-escrowed KASS bond into pass-KASS / fail-KASS, and flips
//! `ai_claim.challenged = 1`. AMM liquidity + trading + TWAP settlement are
//! exercised in tests / Task 11.
//!
//! # Dormant by default
//! A [`Market`] account exists ONLY for a challenged claim. Uncontested claims
//! cost nothing (no account, no CPI) — proven by the test that asserts no
//! Market PDA exists without an `open_challenge` call.
//!
//! # Program-signed KASS split
//! The proposer's bond lives in `oracle.stake_vault`, whose SPL authority is
//! the oracle PDA. The split's `user_underlying_token_account` is that vault and
//! its `authority` is the oracle PDA, signed here with the oracle seeds
//! `[b"oracle", nonce_le, [bump]]`. The pass/fail conditional KASS is minted to
//! two program-controlled token accounts **owned by the oracle PDA** (so Task 11
//! can redeem/merge them on settlement). The `nonce` is supplied in the payload
//! and verified by re-deriving the oracle PDA — the Oracle struct does not store
//! it, and adding a field would re-pin the whole ABI for one signer derivation;
//! verifying the derived PDA matches the passed oracle account is equally safe.
//!
//! # MetaDAO account layout offsets (verified against the deployed v0.4.0 source,
//! `declare_id! == VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg`)
//! * `Question` (8-byte Anchor disc first): `question_id[32]` @8,
//!   `oracle:Pubkey` @40, `payout_numerators: Vec<u32>` len prefix @72. At
//!   `initialize_question` the Vec is `vec![0; num_outcomes]`, so its u32 LE
//!   length == `num_outcomes`.
//! * `ConditionalVault` (disc first): `question:Pubkey` @8,
//!   `underlying_token_mint:Pubkey` @40, `underlying_token_account:Pubkey` @72,
//!   `conditional_token_mints: Vec<Pubkey>` @104.
//!
//! # AMM binding (enforced HERE, at open)
//! Each `pass_amm`/`fail_amm` is fully bound before it is recorded on the
//! `Market` (via [`metadao::assert_amm_bound`]): owned by the AMM program,
//! carrying the `Amm` account discriminator, and whose `base_mint`/`quote_mint`
//! equal this market's pass/fail conditional (KASS, USDC) mints for that outcome;
//! and `pass_amm != fail_amm`. `settle_challenge` re-checks the SAME binding
//! before reading each TWAP. Binding at open is load-bearing: settle pins each
//! AMM to the address recorded here, so a market recorded with an unbindable AMM
//! could never settle — `open_challenge_count` would never return to 0,
//! `finalize_oracle` would be blocked forever, and every stake in the oracle
//! would be permanently locked.
//!
//! # Accounts
//! 0.  oracle              — writable, owned by this program; also the split
//!     authority (signs via the oracle PDA seeds)
//! 1.  ai_claim            — writable, the challenged claim
//! 2.  proposer            — writable, the claim's proposer (source of the bond)
//! 3.  market PDA          — writable, uninitialized (created here)
//! 4.  challenger          — signer, writable; pays the Market rent
//! 5.  question            — read-only MetaDAO question (resolver == oracle PDA)
//! 6.  kass_vault          — writable MetaDAO conditional vault (underlying KASS)
//! 7.  usdc_vault          — read-only MetaDAO conditional vault (underlying USDC)
//! 8.  pass_amm            — read-only, owned by the AMM program
//! 9.  fail_amm            — read-only, owned by the AMM program
//! 10. stake_vault         — writable; == `oracle.stake_vault` (split source)
//! 11. kass_vault_underlying_ata — writable; == kass_vault.underlying_token_account
//! 12. pass_kass_mint      — writable; conditional-token mint idx 0 of kass_vault
//! 13. fail_kass_mint      — writable; conditional-token mint idx 1 of kass_vault
//! 14. oracle_pass_kass    — writable; dest conditional-token acct, owner == oracle PDA
//! 15. oracle_fail_kass    — writable; dest conditional-token acct, owner == oracle PDA
//! 16. conditional_vault program
//! 17. token program
//! 18. system program
//! 19. cv_event_authority  — read-only; conditional_vault `#[event_cpi]` authority
//! 20. protocol            — read-only; the `[b"protocol"]` singleton (`kass_dao` source)
//! 21. kass_dao            — read-only; the futarchy `Dao` (== `protocol.kass_dao`), kass_price source
//! 22. usdc_mint           — read-only; == `oracle.usdc_mint` (escrow vault mint)
//! 23. challenger_usdc_src — writable; challenger's USDC source token account (challenger signs)
//! 24. challenger_usdc_vault — writable, uninit; market-owned USDC escrow created here
//!     at PDA `[b"challenge_usdc", market]`, token authority = oracle PDA
//!
//! # Challenger USDC escrow (Task C1)
//! The escrow is sized via `kass_price` (the governance-anchored futarchy spot
//! TWAP, raw USDC per raw KASS × `1e12`): `required_usdc = bond × twap /
//! KASS_PRICE_SCALE` (u128, overflow-checked), where `bond == proposer.bond`.
//! The cross-decimal (KASS 9dp / USDC 6dp) adjustment is folded into the raw
//! price, so no extra `10^Δdecimals` factor is needed (see
//! [`crate::config::KASS_PRICE_SCALE`]). The amount is computed ON-CHAIN and
//! transferred challenger→escrow; the legacy payload `challenger_usdc` field is
//! gone (it was never trustworthy).
//!
//! # Instruction payload (after the 1-byte discriminant)
//! `oracle_nonce: u64 LE` (exactly 8 bytes).

mod entry;

pub use entry::process;
