//! PDA derivation, keypair loading, and `submit_ai_claim` instruction /
//! transaction construction (the pure build half of the keeper step).

use std::path::Path;

use solana_hash::Hash;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::constants::SUBMIT_AI_CLAIM_PAYLOAD_LEN;
use crate::submit::error::SubmitError;

/// The `submit_ai_claim` instruction discriminant (first `data` byte), tied to
/// the SDK's [`kassandra_oracles_sdk::Ix::SubmitAiClaim`] variant (re-exported from the
/// program) so a renumber in the program breaks this build.
pub const SUBMIT_AI_CLAIM_DISCRIMINANT: u8 = kassandra_oracles_sdk::Ix::SubmitAiClaim as u8;

/// The Kassandra program id as a [`Pubkey`] (from the SDK's canonical constant).
pub fn program_id() -> Pubkey {
    kassandra_oracles_sdk::PROGRAM_ID
}

/// Derive the `ai_claim` PDA (seeds `[b"claim", oracle, proposer]`) via the SDK.
pub fn derive_ai_claim_pda(oracle: &Pubkey, proposer: &Pubkey) -> Pubkey {
    kassandra_oracles_sdk::pda::ai_claim(&program_id(), oracle, proposer).0
}

/// Derive the `proposer` PDA `find_program_address([b"proposer", oracle,
/// authority], kassandra_id)` (the on-chain `propose` contract).
///
/// The keeper is run BY the proposer, so its Proposer PDA is fully determined by
/// the oracle and the signing `authority` (the `--keypair` pubkey) — no separate
/// `--proposer` argument is required.
pub fn derive_proposer_pda(oracle: &Pubkey, authority: &Pubkey) -> Pubkey {
    kassandra_oracles_sdk::pda::proposer(&program_id(), oracle, authority).0
}

/// Build the `submit_ai_claim` [`Instruction`].
///
/// Account metas are in the EXACT processor order
/// (`submit_ai_claim.rs`): `[oracle(w), proposer PDA(w), ai_claim PDA(w),
/// authority(signer,w), system(ro)]`. The `ai_claim` PDA is derived here from
/// `[b"claim", oracle, proposer]`. `data = [SubmitAiClaim disc] ++ payload`
/// (the 97-byte payload the runner already computed — asserted 97 bytes).
pub fn build_submit_ai_claim_ix(
    oracle: &Pubkey,
    proposer: &Pubkey,
    authority: &Pubkey,
    payload: &[u8; SUBMIT_AI_CLAIM_PAYLOAD_LEN],
) -> Instruction {
    // Layout is compile-time-pinned to 97; the runtime assert documents intent.
    assert_eq!(
        payload.len(),
        SUBMIT_AI_CLAIM_PAYLOAD_LEN,
        "submit_ai_claim payload must be exactly {SUBMIT_AI_CLAIM_PAYLOAD_LEN} bytes"
    );

    let ai_claim = derive_ai_claim_pda(oracle, proposer);
    kassandra_oracles_sdk::ix::submit_ai_claim_raw(
        &program_id(),
        *oracle,
        *proposer,
        ai_claim,
        *authority,
        payload,
    )
}

/// Load a Solana CLI JSON keypair file (a 64-byte JSON array: 32 secret ++ 32
/// public) into a [`Keypair`]. Clear errors on a missing file, non-array JSON,
/// wrong length, or bad key bytes.
pub fn load_keypair(path: &Path) -> Result<Keypair, SubmitError> {
    let display = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|e| SubmitError::KeypairRead {
        path: display.clone(),
        message: e.to_string(),
    })?;
    let bytes: Vec<u8> =
        serde_json::from_str(&text).map_err(|e| SubmitError::KeypairMalformed {
            path: display.clone(),
            message: format!("expected a JSON array of 64 bytes: {e}"),
        })?;
    if bytes.len() != 64 {
        return Err(SubmitError::KeypairMalformed {
            path: display,
            message: format!("expected 64 bytes, got {}", bytes.len()),
        });
    }
    Keypair::try_from(&bytes[..]).map_err(|e| SubmitError::KeypairMalformed {
        path: display,
        message: format!("not a valid ed25519 keypair: {e}"),
    })
}

/// Build a legacy [`Message`] (payer = the authority) for the `submit_ai_claim`
/// instruction at `blockhash`, and SIGN it with `authority` (ed25519).
pub fn build_signed_transaction(
    oracle: &Pubkey,
    proposer: &Pubkey,
    authority: &Keypair,
    payload: &[u8; SUBMIT_AI_CLAIM_PAYLOAD_LEN],
    blockhash: Hash,
) -> Transaction {
    let authority_pubkey = authority.pubkey();
    let ix = build_submit_ai_claim_ix(oracle, proposer, &authority_pubkey, payload);
    let message = Message::new_with_blockhash(&[ix], Some(&authority_pubkey), &blockhash);
    Transaction::new(&[authority], message, blockhash)
}

/// Serialize a signed [`Transaction`] to a base64 wire string (bincode → base64)
/// for `sendTransaction` with `encoding: base64`.
pub fn encode_transaction(tx: &Transaction) -> String {
    let bytes = bincode::serialize(tx).expect("bincode-serializing a Transaction is infallible");
    BASE64.encode(bytes)
}
