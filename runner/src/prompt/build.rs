use crate::provider::{CategoricalOptions, CompletionRequest, ModelConfig};

/// The fixed system preamble that frames the task. Part of the canonical
/// assembly (version [`PROMPT_ASSEMBLY_VERSION`]) — changing it changes the
/// hashed bytes and requires a version bump.
pub const SYSTEM_PREAMBLE: &str = "You are an impartial oracle resolver for a categorical prediction market. \
Your task is to determine the single correct outcome by applying the resolution rules to the provided facts. \
Decide based ONLY on the resolution rules and the facts given to you; do not use outside knowledge, assumptions, or information not present below. \
You must choose exactly one option by its integer index.";

/// An agreed fact whose `content` has already been fetched and verified against
/// its on-chain `content_hash` (Task R3 does the fetch + verification; this
/// module accepts the already-verified pair).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    /// The on-chain `content_hash` (sha256 of `content`), 32 bytes. Doubles as
    /// the canonical sort key for deterministic ordering.
    pub content_hash: [u8; 32],
    /// The verified fact content, rendered verbatim into the prompt.
    pub content: String,
}

impl Fact {
    /// Convenience constructor.
    pub fn new(content_hash: [u8; 32], content: impl Into<String>) -> Self {
        Self {
            content_hash,
            content: content.into(),
        }
    }
}

/// The canonical assembled model input: the exact `system` / `user` strings that
/// feed a [`CompletionRequest`] and (via [`crate::hashing::hash_io`]) `io_hash`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledPrompt {
    /// System text: the fixed preamble + the oracle's resolution rules.
    pub system: String,
    /// User text: the canonically-ordered facts + enumerated options + the
    /// answer instruction.
    pub user: String,
}

/// Look up the label for option index `i`, if any. Labels are matched by their
/// explicit `index` field, independent of their position in the vec.
fn label_for(options: &CategoricalOptions, i: u8) -> Option<&str> {
    options
        .labels
        .as_ref()
        .and_then(|ls| ls.iter().find(|o| o.index == i))
        .and_then(|o| o.label.as_deref())
}

/// Assemble the canonical `system` / `user` strings.
///
/// `interpretation` is the oracle's resolution-rule text (committed on-chain via
/// `prompt_hash`). `facts` are the already-verified `(content_hash, content)`
/// pairs in ANY order — they are sorted canonically here. `options` is the
/// categorical answer space.
///
/// ## `system` layout
///
/// ```text
/// {SYSTEM_PREAMBLE}
///
/// # Resolution rules
///
/// {interpretation}
/// ```
///
/// ## `user` layout
///
/// Three blocks joined by `"\n\n"`, no trailing newline:
///
/// ```text
/// # Facts
///
/// ## Fact 1 (sha256: {hex64})
/// {content}
///
/// ## Fact 2 (sha256: {hex64})
/// {content}
///
/// # Options
///
/// You must choose exactly one of the following options by its integer index:
///
/// [0] {label or "(no label)"}
/// [1] {label or "(no label)"}
///
/// # Answer
///
/// Respond with the structured JSON output { "option_index": <index> }, ...
/// ```
///
/// Facts are sorted by `content_hash` ascending and numbered 1..=N in that
/// order; each is tagged with its `content_hash` hex so the fact set is
/// unambiguous (two distinct fact sets cannot render to the same bytes). If
/// there are no facts, the body is the literal `(no facts provided)`.
pub fn assemble(
    interpretation: &str,
    facts: &[Fact],
    options: &CategoricalOptions,
) -> AssembledPrompt {
    let system = format!("{SYSTEM_PREAMBLE}\n\n# Resolution rules\n\n{interpretation}");

    // Canonical fact order: sort references by content_hash bytes ascending.
    let mut ordered: Vec<&Fact> = facts.iter().collect();
    ordered.sort_by(|a, b| a.content_hash.cmp(&b.content_hash));

    let facts_block = if ordered.is_empty() {
        "# Facts\n\n(no facts provided)".to_string()
    } else {
        let entries: Vec<String> = ordered
            .iter()
            .enumerate()
            .map(|(i, f)| {
                format!(
                    "## Fact {} (sha256: {})\n{}",
                    i + 1,
                    hex::encode(&f.content_hash),
                    f.content
                )
            })
            .collect();
        format!("# Facts\n\n{}", entries.join("\n\n"))
    };

    let count = options.count;
    let option_lines: Vec<String> = (0..count)
        .map(|i| match label_for(options, i) {
            Some(label) => format!("[{i}] {label}"),
            None => format!("[{i}] (no label)"),
        })
        .collect();
    let options_block = format!(
        "# Options\n\nYou must choose exactly one of the following options by its integer index:\n\n{}",
        option_lines.join("\n")
    );

    // count is >= 2 on-chain; saturating_sub guards a degenerate count == 0.
    let max_index = count.saturating_sub(1);
    let answer_block = format!(
        "# Answer\n\nRespond with the structured JSON output {{ \"option_index\": <index> }}, \
where <index> is the integer index (0 to {max_index} inclusive) of the single correct option. \
Base your choice ONLY on the resolution rules and the facts above."
    );

    let user = [facts_block, options_block, answer_block].join("\n\n");

    AssembledPrompt { system, user }
}

/// Assemble + wrap into a ready-to-send [`CompletionRequest`] (convenience for
/// Tasks R4/R5). The `config`'s `provider`/`model_id`/`thinking`/`max_tokens`
/// flow through to `params_hash`; the assembled `system`/`user` flow through to
/// `io_hash`.
pub fn build_request(
    interpretation: &str,
    facts: &[Fact],
    options: CategoricalOptions,
    config: ModelConfig,
) -> CompletionRequest {
    let AssembledPrompt { system, user } = assemble(interpretation, facts, &options);
    CompletionRequest {
        system,
        user,
        options,
        config,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::parse_option_index;
    use crate::provider::AiProvider;
    use crate::provider::{CategoricalOption, CategoricalOptions, MockProvider};

    fn h(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn opts_no_labels(count: u8) -> CategoricalOptions {
        CategoricalOptions {
            count,
            labels: None,
        }
    }

    fn opts_with_labels() -> CategoricalOptions {
        CategoricalOptions {
            count: 2,
            labels: Some(vec![
                CategoricalOption {
                    index: 0,
                    label: Some("Yes".to_string()),
                },
                CategoricalOption {
                    index: 1,
                    label: Some("No".to_string()),
                },
            ]),
        }
    }

    // --- assembly determinism ----------------------------------------------

    #[test]
    fn assembly_is_independent_of_fact_input_order() {
        let interp = "Resolve YES iff the event occurred before the deadline.";
        let opts = opts_with_labels();
        let f_a = Fact::new(h(0x01), "alpha fact");
        let f_b = Fact::new(h(0x02), "beta fact");
        let f_c = Fact::new(h(0x03), "gamma fact");

        let forward = assemble(interp, &[f_a.clone(), f_b.clone(), f_c.clone()], &opts);
        let shuffled = assemble(interp, &[f_c, f_a, f_b], &opts);

        // Different input order -> byte-identical output (proves content_hash sort).
        assert_eq!(forward.system, shuffled.system);
        assert_eq!(forward.user, shuffled.user);
    }

    #[test]
    fn facts_render_in_content_hash_order() {
        let interp = "rules";
        let opts = opts_no_labels(2);
        // Insert out of order; expect rendered order 0x01, 0x05, 0xaa.
        let facts = vec![
            Fact::new(h(0xaa), "third"),
            Fact::new(h(0x01), "first"),
            Fact::new(h(0x05), "second"),
        ];
        let user = assemble(interp, &facts, &opts).user;
        let p_first = user.find("first").unwrap();
        let p_second = user.find("second").unwrap();
        let p_third = user.find("third").unwrap();
        assert!(p_first < p_second && p_second < p_third);
        // Numbered 1..=N in canonical order.
        assert!(user.contains("## Fact 1 (sha256: 0101010101010101010101010101010101010101010101010101010101010101)\nfirst"));
        assert!(user.contains("## Fact 3 (sha256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)\nthird"));
    }

    #[test]
    fn options_enumerated_with_labels() {
        let user = assemble("r", &[], &opts_with_labels()).user;
        assert!(user.contains("[0] Yes\n[1] No"));
    }

    #[test]
    fn options_enumerated_without_labels() {
        let user = assemble("r", &[], &opts_no_labels(3)).user;
        assert!(user.contains("[0] (no label)\n[1] (no label)\n[2] (no label)"));
    }

    #[test]
    fn no_facts_renders_placeholder() {
        let user = assemble("r", &[], &opts_no_labels(2)).user;
        assert!(user.contains("# Facts\n\n(no facts provided)"));
    }

    #[test]
    fn system_contains_preamble_then_rules() {
        let a = assemble("MY RULES", &[], &opts_no_labels(2));
        assert_eq!(
            a.system,
            format!("{SYSTEM_PREAMBLE}\n\n# Resolution rules\n\nMY RULES")
        );
    }

    #[test]
    fn no_trailing_newline_or_whitespace() {
        let a = assemble("rules", &[Fact::new(h(1), "c")], &opts_with_labels());
        assert_eq!(
            a.system.trim_end(),
            a.system,
            "system has trailing whitespace"
        );
        assert_eq!(a.user.trim_end(), a.user, "user has trailing whitespace");
    }

    // --- regression anchor: pin the EXACT assembled bytes ------------------
    // A change to ANY part of the format flips these strings. If this test
    // fails, the assembly changed: update the strings AND bump
    // PROMPT_ASSEMBLY_VERSION in hashing.rs (claims would otherwise silently
    // hash differently under the same version).

    #[test]
    fn assembly_regression_anchor() {
        let interp = "Resolve YES if BTC closed above $100k on the date; otherwise NO.";
        let facts = vec![
            // Deliberately out of content_hash order.
            Fact::new(h(0x22), "BTC closed at $98,000."),
            Fact::new(h(0x11), "The date in question is 2025-12-31."),
        ];
        let opts = opts_with_labels();
        let a = assemble(interp, &facts, &opts);

        let expected_system = "You are an impartial oracle resolver for a categorical prediction market. \
Your task is to determine the single correct outcome by applying the resolution rules to the provided facts. \
Decide based ONLY on the resolution rules and the facts given to you; do not use outside knowledge, assumptions, or information not present below. \
You must choose exactly one option by its integer index.\n\n\
# Resolution rules\n\n\
Resolve YES if BTC closed above $100k on the date; otherwise NO.";
        assert_eq!(a.system, expected_system);

        let expected_user = "# Facts\n\n\
## Fact 1 (sha256: 1111111111111111111111111111111111111111111111111111111111111111)\n\
The date in question is 2025-12-31.\n\n\
## Fact 2 (sha256: 2222222222222222222222222222222222222222222222222222222222222222)\n\
BTC closed at $98,000.\n\n\
# Options\n\n\
You must choose exactly one of the following options by its integer index:\n\n\
[0] Yes\n[1] No\n\n\
# Answer\n\n\
Respond with the structured JSON output { \"option_index\": <index> }, \
where <index> is the integer index (0 to 1 inclusive) of the single correct option. \
Base your choice ONLY on the resolution rules and the facts above.";
        assert_eq!(a.user, expected_user);
    }

    // --- pipeline composition (assemble -> request -> mock -> parse) --------

    #[tokio::test]
    async fn pipeline_composes_through_mock_provider() {
        let opts = opts_with_labels();
        let req = build_request(
            "Resolve per the rules.",
            &[Fact::new(h(0x01), "fact one")],
            opts,
            ModelConfig {
                model_id: "claude-opus-4-8".to_string(),
                provider: "mock".to_string(),
                max_tokens: 1024,
                thinking: Some("adaptive".to_string()),
            },
        );

        let provider = MockProvider::new(1, r#"{"option_index":1}"#, "mock-claude");
        let resp = provider.complete(&req).await.unwrap();

        // The raw response parses back to the same option index, validated
        // against the request's option count.
        let parsed = parse_option_index(&resp.raw_response, req.options.count).unwrap();
        assert_eq!(parsed, 1);
        assert_eq!(parsed, resp.option_index);
    }
}
