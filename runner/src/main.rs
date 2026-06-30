//! `kassandra-runner` CLI entrypoint (stub).
//!
//! The `run` / `verify` subcommands are wired in Task R4. For R0 this is a
//! placeholder so the binary target exists; the library carries the real logic.

fn main() {
    eprintln!(
        "kassandra-runner {}: CLI not wired yet (Task R4). Use the library API.",
        env!("CARGO_PKG_VERSION")
    );
}
