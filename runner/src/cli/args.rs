//! The clap CLI surface + argument parsing/dispatch.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::anthropic::AnthropicProvider;
use crate::fetch::HttpFactFetcher;
use crate::provider::{AiProvider, MockProvider};
use crate::submit::ConfirmOptions;

use crate::cli::{
    build_model_config, resolve_config, resolve_submit_target, run_core, submit_claim, use_mock,
    verify_core, SubmittedClaim,
};

// --- clap -------------------------------------------------------------------

/// The Kassandra off-chain AI runner CLI.
#[derive(Debug, Parser)]
#[command(name = "kassandra-runner", version, about)]
pub struct Cli {
    /// The subcommand.
    #[command(subcommand)]
    pub command: Command,
}

/// The runner subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Resolve an oracle: fetch + verify facts, call the model, emit the claim
    /// metadata + the 97-byte submit_ai_claim payload as JSON.
    Run(RunArgs),
    /// Re-run for the same config and compare the produced option to a submitted
    /// claim's option; advise whether to challenge.
    Verify(VerifyArgs),
}

/// Shared provider/config options.
#[derive(Debug, Parser)]
pub struct CommonArgs {
    /// Path to the JSON config; if omitted (and no `--oracle`), read from stdin.
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Build the config from an on-chain oracle (base58 pubkey) instead of a
    /// JSON `--config`: the oracle's `options_count`/`deadline`/agreed facts are
    /// read over RPC, and the interpretation comes from `--prompt-file` (whose
    /// sha256 must equal the on-chain `prompt_hash`). Requires `--rpc-url` +
    /// `--prompt-file`; mutually exclusive with `--config`.
    #[arg(long)]
    pub oracle: Option<String>,
    /// Solana JSON-RPC url used with `--oracle`.
    #[arg(long)]
    pub rpc_url: Option<String>,
    /// Path to the interpretation prompt-text file used with `--oracle`; its
    /// sha256 must equal the on-chain `oracle.prompt_hash` (else the run is
    /// rejected).
    #[arg(long)]
    pub prompt_file: Option<PathBuf>,
    /// Use the deterministic MockProvider (offline; no API key needed). Also
    /// enabled by setting KASSANDRA_RUNNER_MOCK.
    #[arg(long)]
    pub mock: bool,
    /// Override the pinned model string (default: claude-opus-4-8).
    #[arg(long)]
    pub model: Option<String>,
    /// Override max_tokens (default: 4096).
    #[arg(long)]
    pub max_tokens: Option<u32>,
}

/// `run` arguments.
#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Shared options.
    #[command(flatten)]
    pub common: CommonArgs,
    /// Keeper mode: after producing the claim, SIGN + SEND + CONFIRM the
    /// `submit_ai_claim` transaction on chain (default: emit-only, no network
    /// write). Requires `--keypair` and `--rpc-url`; the signer MUST be the
    /// proposer's authority. The oracle comes from `--oracle` or the config's
    /// `oracle` field; the Proposer PDA is derived from it + the keypair pubkey.
    #[arg(long)]
    pub submit: bool,
    /// Path to the Solana CLI keypair JSON (a 64-byte array) that signs the
    /// `submit_ai_claim` transaction in `--submit` mode. This keypair MUST be
    /// the proposer's registered `authority`.
    #[arg(long)]
    pub keypair: Option<PathBuf>,
}

/// `verify` arguments.
#[derive(Debug, Parser)]
pub struct VerifyArgs {
    /// Shared options.
    #[command(flatten)]
    pub common: CommonArgs,
    /// The submitted claim's categorical option to compare against.
    #[arg(long)]
    pub option: u8,
    /// Optional submitted model_id (hex) to compare.
    #[arg(long)]
    pub submitted_model_id: Option<String>,
    /// Optional submitted params_hash (hex) to compare.
    #[arg(long)]
    pub submitted_params_hash: Option<String>,
    /// Optional submitted io_hash (hex) to compare.
    #[arg(long)]
    pub submitted_io_hash: Option<String>,
}

/// Build the chosen provider (mock or real Anthropic).
fn build_provider(mock: bool) -> anyhow::Result<Box<dyn AiProvider>> {
    if use_mock(mock) {
        Ok(Box::new(MockProvider::default()))
    } else {
        Ok(Box::new(AnthropicProvider::from_env()?))
    }
}

/// Parse args and dispatch. `main` calls this inside `#[tokio::main]`.
pub async fn run_cli() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run(args) => {
            let config = resolve_config(&args.common).await?;
            // Validate `--submit` args BEFORE the (paid) model call so a missing
            // --keypair / --rpc-url / oracle fails fast.
            let submit_target =
                resolve_submit_target(&args.common, args.submit, args.keypair.as_deref(), &config)?;
            let model_config = build_model_config(args.common.model, args.common.max_tokens);
            let fetcher = HttpFactFetcher::new()?;
            let provider = build_provider(args.common.mock)?;
            let mut out = run_core(&config, model_config, &fetcher, provider.as_ref()).await?;

            if let Some(target) = submit_target {
                let authority = crate::submit::load_keypair(&target.keypair_path)?;
                let rpc = crate::rpc::HttpJsonRpc::new(target.rpc_url)?;
                // Reuse the run's OWN payload bytes (never recomputed) so the
                // submitted claim can never diverge from the emitted metadata.
                let submission = submit_claim(
                    &rpc,
                    &target.oracle,
                    &authority,
                    &out.submit_ai_claim_payload,
                    ConfirmOptions::default(),
                )
                .await?;
                out.submission = Some(submission);
            }

            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Command::Verify(args) => {
            let config = resolve_config(&args.common).await?;
            let model_config = build_model_config(args.common.model, args.common.max_tokens);
            let fetcher = HttpFactFetcher::new()?;
            let provider = build_provider(args.common.mock)?;
            let submitted = SubmittedClaim {
                option: args.option,
                model_id_hex: args.submitted_model_id,
                params_hash_hex: args.submitted_params_hash,
                io_hash_hex: args.submitted_io_hash,
            };
            let out = verify_core(
                &config,
                model_config,
                &fetcher,
                provider.as_ref(),
                &submitted,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
    }
    Ok(())
}
