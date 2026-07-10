use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use serde_json::json;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::constants::SUBMIT_AI_CLAIM_PAYLOAD_LEN;
use crate::rpc::MockRpc;
use crate::submit::*;

/// A fresh ed25519 keypair for the sign/build tests. Each test uses one
/// instance consistently; no fixed vector is asserted, so a random key is
/// fine (the tests check the pubkey round-trips and the signature verifies).
fn sample_keypair() -> Keypair {
    Keypair::new()
}

fn sample_keypair_json(kp: &Keypair) -> String {
    let bytes = kp.to_bytes(); // [u8; 64]
    serde_json::to_string(&bytes.to_vec()).unwrap()
}

fn oracle_pk() -> Pubkey {
    Pubkey::new_from_array([1u8; 32])
}
fn proposer_pk() -> Pubkey {
    Pubkey::new_from_array([2u8; 32])
}

fn sample_payload(option: u8) -> [u8; SUBMIT_AI_CLAIM_PAYLOAD_LEN] {
    // model_id = 0x11.., params_hash = 0x22.., io_hash = 0x33.., option.
    let mut p = [0u8; SUBMIT_AI_CLAIM_PAYLOAD_LEN];
    p[0..32].fill(0x11);
    p[32..64].fill(0x22);
    p[64..96].fill(0x33);
    p[96] = option;
    p
}

// --- discriminant + system program pins ---------------------------------

#[test]
fn discriminant_is_submit_ai_claim() {
    assert_eq!(SUBMIT_AI_CLAIM_DISCRIMINANT, 3);
}

#[test]
fn system_program_id_is_canonical() {
    assert_eq!(
        kassandra_oracles_sdk::SYSTEM_PROGRAM_ID,
        Pubkey::from_str("11111111111111111111111111111111").unwrap()
    );
}

// --- instruction builder ------------------------------------------------

#[test]
fn ix_builder_has_exact_metas_and_data() {
    let oracle = oracle_pk();
    let proposer = proposer_pk();
    let authority = sample_keypair().pubkey();
    let payload = sample_payload(1);

    let ix = build_submit_ai_claim_ix(&oracle, &proposer, &authority, &payload);

    // Program id is the kassandra id (pinocchio [u8;32] → Pubkey).
    assert_eq!(ix.program_id, program_id());
    assert_eq!(ix.program_id, kassandra_oracles_sdk::PROGRAM_ID);

    // EXACT processor account order + roles.
    let ai_claim = derive_ai_claim_pda(&oracle, &proposer);
    let expected = [
        (oracle, false, true),                            // oracle (w)
        (proposer, false, true),                          // proposer PDA (w)
        (ai_claim, false, true),                          // ai_claim PDA (w)
        (authority, true, true),                          // authority (signer, w)
        (kassandra_oracles_sdk::SYSTEM_PROGRAM_ID, false, false), // system (ro)
    ];
    assert_eq!(ix.accounts.len(), expected.len());
    for (meta, (pk, signer, writable)) in ix.accounts.iter().zip(expected) {
        assert_eq!(meta.pubkey, pk);
        assert_eq!(meta.is_signer, signer);
        assert_eq!(meta.is_writable, writable);
    }

    // data == [disc=3] ++ 97-byte payload.
    assert_eq!(ix.data.len(), 1 + SUBMIT_AI_CLAIM_PAYLOAD_LEN);
    assert_eq!(ix.data[0], 3);
    assert_eq!(&ix.data[1..], &payload[..]);
}

#[test]
fn ai_claim_pda_matches_claim_oracle_proposer_seeds() {
    let oracle = oracle_pk();
    let proposer = proposer_pk();
    let expected = Pubkey::find_program_address(
        &[b"claim", oracle.as_ref(), proposer.as_ref()],
        &program_id(),
    )
    .0;
    assert_eq!(derive_ai_claim_pda(&oracle, &proposer), expected);
}

#[test]
fn proposer_pda_matches_proposer_oracle_authority_seeds() {
    let oracle = oracle_pk();
    let authority = sample_keypair().pubkey();
    let expected = Pubkey::find_program_address(
        &[b"proposer", oracle.as_ref(), authority.as_ref()],
        &program_id(),
    )
    .0;
    assert_eq!(derive_proposer_pda(&oracle, &authority), expected);
}

#[test]
fn ix_carries_the_runoutput_payload_bytes() {
    // The 97-byte payload the runner emits (via ClaimMetadata::to_payload)
    // must land verbatim in the instruction data.
    use crate::hashing::ClaimMetadata;
    let meta = ClaimMetadata {
        model_id: [0xaa; 32],
        params_hash: [0xbb; 32],
        io_hash: [0xcc; 32],
    };
    let payload = meta.to_payload(2);
    let ix = build_submit_ai_claim_ix(
        &oracle_pk(),
        &proposer_pk(),
        &sample_keypair().pubkey(),
        &payload,
    );
    assert_eq!(&ix.data[1..], &payload[..]);
    assert_eq!(&ix.data[1..33], &[0xaa; 32]);
    assert_eq!(&ix.data[33..65], &[0xbb; 32]);
    assert_eq!(&ix.data[65..97], &[0xcc; 32]);
    assert_eq!(ix.data[97], 2);
}

// --- keypair loader -----------------------------------------------------

#[test]
fn load_keypair_parses_64_byte_json_array() {
    let kp = sample_keypair();
    let json = sample_keypair_json(&kp);
    let dir = std::env::temp_dir();
    let path = dir.join(format!("kass-test-kp-{}.json", std::process::id()));
    std::fs::write(&path, json).unwrap();

    let loaded = load_keypair(&path).unwrap();
    assert_eq!(loaded.pubkey(), kp.pubkey());

    std::fs::remove_file(&path).ok();
}

#[test]
fn load_keypair_rejects_missing_file() {
    let err = load_keypair(Path::new("/no/such/kass-keypair.json")).unwrap_err();
    assert!(matches!(err, SubmitError::KeypairRead { .. }), "{err}");
}

#[test]
fn load_keypair_rejects_wrong_length() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("kass-test-badkp-{}.json", std::process::id()));
    std::fs::write(&path, "[1,2,3]").unwrap();
    let err = load_keypair(&path).unwrap_err();
    assert!(matches!(err, SubmitError::KeypairMalformed { .. }), "{err}");
    std::fs::remove_file(&path).ok();
}

// --- message build + sign -----------------------------------------------

#[test]
fn signed_tx_verifies_and_carries_blockhash_and_payer() {
    let authority = sample_keypair();
    let oracle = oracle_pk();
    let proposer = proposer_pk();
    let payload = sample_payload(1);
    let blockhash = Hash::new_from_array([9u8; 32]);

    let tx = build_signed_transaction(&oracle, &proposer, &authority, &payload, blockhash);

    // The ed25519 signature verifies against the payer pubkey.
    assert!(tx.verify().is_ok(), "signature must verify");
    // Payer (fee payer / first account key) is the authority.
    assert_eq!(tx.message.account_keys[0], authority.pubkey());
    // The message carries the fetched blockhash.
    assert_eq!(tx.message.recent_blockhash, blockhash);
    // Exactly one signer (the authority); `verify()` above already proved
    // the signature is a real ed25519 sig over the message (not a default).
    assert_eq!(tx.signatures.len(), 1);
}

#[test]
fn encode_transaction_is_base64_bincode() {
    let authority = sample_keypair();
    let tx = build_signed_transaction(
        &oracle_pk(),
        &proposer_pk(),
        &authority,
        &sample_payload(0),
        Hash::new_from_array([5u8; 32]),
    );
    let encoded = encode_transaction(&tx);
    let decoded = BASE64.decode(&encoded).unwrap();
    let roundtrip: Transaction = bincode::deserialize(&decoded).unwrap();
    assert_eq!(roundtrip, tx);
}

// --- send + confirm flow (offline via MockRpc) --------------------------

fn fast_opts() -> ConfirmOptions {
    ConfirmOptions {
        max_polls: 3,
        poll_interval: Duration::from_millis(0),
        require_finalized: false,
    }
}

#[tokio::test]
async fn get_latest_blockhash_decodes_base58() {
    let bh = Hash::new_from_array([4u8; 32]);
    let rpc = MockRpc::new().with(
        "getLatestBlockhash",
        json!({ "context": { "slot": 1 }, "value": { "blockhash": bh.to_string(), "lastValidBlockHeight": 100 } }),
    );
    let got = get_latest_blockhash(&rpc).await.unwrap();
    assert_eq!(got, bh);
}

#[tokio::test]
async fn submit_and_confirm_happy_path() {
    let sig = "5".repeat(64); // a stand-in base58 signature string
    let rpc = MockRpc::new()
        .with(
            "getLatestBlockhash",
            json!({ "context": { "slot": 1 }, "value": { "blockhash": Hash::new_from_array([1u8;32]).to_string(), "lastValidBlockHeight": 100 } }),
        )
        .with("sendTransaction", json!(sig))
        .with(
            "getSignatureStatuses",
            json!({ "context": { "slot": 2 }, "value": [ { "slot": 2, "confirmations": null, "err": null, "confirmationStatus": "confirmed" } ] }),
        );

    let out = submit_and_confirm(
        &rpc,
        &oracle_pk(),
        &proposer_pk(),
        &sample_keypair(),
        &sample_payload(1),
        fast_opts(),
    )
    .await
    .unwrap();
    assert_eq!(out.signature, sig);
    assert_eq!(out.confirmation_status, "confirmed");
}

#[tokio::test]
async fn confirm_surfaces_failed_tx_err() {
    let sig = "F".repeat(64);
    let rpc = MockRpc::new().with(
        "getSignatureStatuses",
        json!({ "context": { "slot": 2 }, "value": [ { "slot": 2, "confirmations": null, "err": { "InstructionError": [0, { "Custom": 7 }] }, "confirmationStatus": "processed" } ] }),
    );
    let err = confirm(&rpc, &sig, fast_opts()).await.unwrap_err();
    match err {
        SubmitError::TxFailed { signature, error } => {
            assert_eq!(signature, sig);
            assert!(error.contains("InstructionError"), "{error}");
        }
        other => panic!("expected TxFailed, got {other}"),
    }
}

#[tokio::test]
async fn confirm_times_out_when_never_seen() {
    let sig = "N".repeat(64);
    // Always `null` (never seen) → poll budget exhausts.
    let rpc = MockRpc::new().with(
        "getSignatureStatuses",
        json!({ "context": { "slot": 2 }, "value": [ null ] }),
    );
    let err = confirm(&rpc, &sig, fast_opts()).await.unwrap_err();
    assert!(matches!(err, SubmitError::ConfirmTimeout { .. }), "{err}");
}

#[tokio::test]
async fn send_transaction_surfaces_preflight_jsonrpc_error() {
    // A preflight failure comes back as a JSON-RPC error from the transport.
    // MockRpc only serves `result`s, so model the RPC error via an empty
    // mock (no canned `sendTransaction`) → Malformed-ish RpcError surfaced.
    let rpc = MockRpc::new();
    let err = send_transaction(&rpc, "AA==").await.unwrap_err();
    assert!(matches!(err, SubmitError::Rpc(_)), "{err}");
}
