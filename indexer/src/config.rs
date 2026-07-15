//! Small env-config helpers used to wire up the indexer in `main`.

use std::str::FromStr;

/// Parse an env var into `T`, falling back to `default` when unset or unparseable.
pub fn env_num<T: FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// The websocket url for the price subscriber: `SOLANA_WS_URL` if set, else derive
/// it from the HTTP RPC url — `http`→`ws` / `https`→`wss`, and when the url carries
/// an explicit port, use port+1 (the Solana pubsub convention that local validators
/// and surfpool follow: RPC 8899 → WS 8900). Returns `None` for a URL we can't map
/// (no explicit port on an https provider that needs an out-of-band ws host — set
/// `SOLANA_WS_URL` there).
pub fn ws_url_for_prices(rpc_url: &str) -> Option<String> {
    if let Ok(explicit) = std::env::var("SOLANA_WS_URL") {
        if !explicit.is_empty() {
            return Some(explicit);
        }
    }
    let (scheme, rest, ws_scheme) = if let Some(r) = rpc_url.strip_prefix("http://") {
        ("http://", r, "ws://")
    } else if let Some(r) = rpc_url.strip_prefix("https://") {
        ("https://", r, "wss://")
    } else {
        return None;
    };
    let _ = scheme;
    // Split host[:port][/path]; we only remap an explicit host:port authority.
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let (host, port) = authority.rsplit_once(':')?;
    let next = port.parse::<u16>().ok()?.checked_add(1)?;
    Some(format!("{ws_scheme}{host}:{next}{path}"))
}

#[cfg(test)]
mod ws_url_tests {
    use super::ws_url_for_prices;

    #[test]
    fn derives_ws_from_http_with_port() {
        // No SOLANA_WS_URL in the test env → derivation path (http→ws, port+1).
        assert_eq!(
            ws_url_for_prices("http://127.0.0.1:8960").as_deref(),
            Some("ws://127.0.0.1:8961")
        );
        assert_eq!(
            ws_url_for_prices("https://rpc.example.com:443/path").as_deref(),
            Some("wss://rpc.example.com:444/path")
        );
        // No explicit port → not derivable (caller must set SOLANA_WS_URL).
        assert_eq!(ws_url_for_prices("https://rpc.example.com"), None);
        assert_eq!(ws_url_for_prices("garbage"), None);
    }
}
