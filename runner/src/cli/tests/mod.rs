use super::{FactInput, OptionLabelInput, RunnerConfig};
use sha2::{Digest, Sha256};

mod core;
mod on_chain;
mod submit;

fn sha256_hex(content: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(content);
    hex::encode(&h.finalize())
}

fn sample_config(uri: &str, content: &[u8]) -> RunnerConfig {
    RunnerConfig {
        interpretation: "Resolve YES if BTC closed above $100k; otherwise NO.".to_string(),
        options_count: 2,
        option_labels: Some(vec![
            OptionLabelInput {
                index: 0,
                label: "Yes".to_string(),
            },
            OptionLabelInput {
                index: 1,
                label: "No".to_string(),
            },
        ]),
        facts: vec![FactInput {
            content_hash: sha256_hex(content),
            uri: uri.to_string(),
        }],
        oracle: None,
        proposer: None,
    }
}

fn parse_payload(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}
