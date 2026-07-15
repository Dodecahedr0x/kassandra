use crate::fetch::types::sha256;
use crate::fetch::*;
use crate::provider::{AiProvider, CategoricalOptions, MockProvider};

/// Real `content_hash` for some content: `sha256(content)`.
fn ch(content: &[u8]) -> [u8; 32] {
    sha256(content)
}

// --- match --------------------------------------------------------------

#[tokio::test]
async fn verifies_matching_content() {
    let content = b"BTC closed at $98,000 on 2025-12-31.";
    let uri = "https://facts.example/btc";
    let fetcher = MockFactFetcher::new().with(uri, content.to_vec());
    let fact_ref = FactRef::new(ch(content), uri);

    let fact = fetch_and_verify_fact(&fetcher, &fact_ref).await.unwrap();

    // The verified Fact carries the content_hash and the UTF-8 of the body.
    assert_eq!(fact.content_hash, ch(content));
    assert_eq!(fact.content, "BTC closed at $98,000 on 2025-12-31.");
}

// --- mismatch (tampered content) ----------------------------------------

#[tokio::test]
async fn rejects_content_hash_mismatch() {
    let committed = b"the agreed fact";
    let tampered = b"a DIFFERENT, tampered fact";
    let uri = "https://facts.example/x";
    // The fetcher serves tampered bytes, but the ref commits to the
    // original content's hash.
    let fetcher = MockFactFetcher::new().with(uri, tampered.to_vec());
    let fact_ref = FactRef::new(ch(committed), uri);

    let err = fetch_and_verify_fact(&fetcher, &fact_ref)
        .await
        .unwrap_err();

    match err {
        VerifyError::ContentHashMismatch {
            uri: u,
            expected,
            actual,
        } => {
            assert_eq!(u, uri);
            assert_eq!(expected, hex::encode(&ch(committed)));
            assert_eq!(actual, hex::encode(&ch(tampered)));
            assert_ne!(expected, actual);
        }
        other => panic!("expected ContentHashMismatch, got {other:?}"),
    }
}

// --- fetch failure ------------------------------------------------------

#[tokio::test]
async fn surfaces_fetch_failure_with_uri() {
    let uri = "https://facts.example/missing";
    let fetcher = MockFactFetcher::new(); // nothing registered
    let fact_ref = FactRef::new([0u8; 32], uri);

    let err = fetch_and_verify_fact(&fetcher, &fact_ref)
        .await
        .unwrap_err();

    // The uri is named in the rendered error.
    assert!(format!("{err}").contains(uri));
    match err {
        VerifyError::Fetch(FetchError::NotFound { uri: u }) => assert_eq!(u, uri),
        other => panic!("expected Fetch(NotFound), got {other:?}"),
    }
}

// --- non-UTF-8 ----------------------------------------------------------

#[tokio::test]
async fn rejects_non_utf8_body_that_hashes_correctly() {
    // Invalid UTF-8 bytes, but the ref commits to THEIR hash, so the hash
    // check passes and the UTF-8 check is what must reject it.
    let bytes: Vec<u8> = vec![0xff, 0xfe, 0x00, 0x80];
    let uri = "https://facts.example/binary";
    let fetcher = MockFactFetcher::new().with(uri, bytes.clone());
    let fact_ref = FactRef::new(ch(&bytes), uri);

    let err = fetch_and_verify_fact(&fetcher, &fact_ref)
        .await
        .unwrap_err();

    match err {
        VerifyError::NonUtf8 { uri: u, .. } => assert_eq!(u, uri),
        other => panic!("expected NonUtf8, got {other:?}"),
    }
}

// --- unsupported scheme (HTTP fetcher, offline — fails before any I/O) ---

#[tokio::test]
async fn http_fetcher_rejects_non_http_scheme() {
    let fetcher = HttpFactFetcher::new().unwrap();
    let err = fetcher.fetch("file:///etc/passwd").await.unwrap_err();
    match err {
        FetchError::UnsupportedScheme { uri, scheme } => {
            assert_eq!(uri, "file:///etc/passwd");
            assert_eq!(scheme, "file");
        }
        other => panic!("expected UnsupportedScheme, got {other:?}"),
    }
}

// --- body-size cap (local server, offline) ------------------------------

#[tokio::test]
async fn http_fetcher_rejects_oversize_body() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // A one-shot server that returns a body larger than the cap we set.
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        // Drain the request headers so reqwest's write completes.
        let mut buf = [0u8; 1024];
        let _ = sock.read(&mut buf).await;
        let body = vec![b'x'; 1000];
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n",
            body.len()
        );
        let _ = sock.write_all(head.as_bytes()).await;
        let _ = sock.write_all(&body).await;
        let _ = sock.flush().await;
    });

    let fetcher = HttpFactFetcher::new().unwrap().with_max_body_bytes(100);
    let uri = format!("http://{addr}/big");
    let err = fetcher.fetch(&uri).await.unwrap_err();
    match err {
        FetchError::TooLarge { uri: u, limit } => {
            assert_eq!(u, uri);
            assert_eq!(limit, 100);
        }
        other => panic!("expected TooLarge, got {other:?}"),
    }
}

// --- multiple facts, fail-fast ------------------------------------------

#[tokio::test]
async fn batch_all_good_returns_verified_in_order() {
    let c0 = b"fact zero";
    let c1 = b"fact one";
    let c2 = b"fact two";
    let fetcher = MockFactFetcher::new()
        .with("https://f/0", c0.to_vec())
        .with("https://f/1", c1.to_vec())
        .with("https://f/2", c2.to_vec());
    let refs = vec![
        FactRef::new(ch(c0), "https://f/0"),
        FactRef::new(ch(c1), "https://f/1"),
        FactRef::new(ch(c2), "https://f/2"),
    ];

    let facts = fetch_and_verify_facts(&fetcher, &refs).await.unwrap();

    assert_eq!(facts.len(), 3);
    // Same order as the input refs.
    assert_eq!(facts[0].content, "fact zero");
    assert_eq!(facts[1].content, "fact one");
    assert_eq!(facts[2].content, "fact two");
}

#[tokio::test]
async fn batch_fails_fast_on_one_bad_fact() {
    let good = b"good fact";
    let committed_for_bad = b"committed content";
    let tampered = b"tampered content";
    let fetcher = MockFactFetcher::new()
        .with("https://f/good", good.to_vec())
        .with("https://f/bad", tampered.to_vec());
    let refs = vec![
        FactRef::new(ch(good), "https://f/good"),
        // This one is tampered: serves bytes that don't match its hash.
        FactRef::new(ch(committed_for_bad), "https://f/bad"),
    ];

    let err = fetch_and_verify_facts(&fetcher, &refs).await.unwrap_err();
    match err {
        VerifyError::ContentHashMismatch { uri, .. } => assert_eq!(uri, "https://f/bad"),
        other => panic!("expected ContentHashMismatch, got {other:?}"),
    }
}

// --- composition: verified facts feed R2's assemble ---------------------

#[tokio::test]
async fn verified_facts_feed_prompt_assembly_and_mock_provider() {
    let c0 = b"The date in question is 2025-12-31.";
    let c1 = b"BTC closed at $98,000.";
    let fetcher = MockFactFetcher::new()
        .with("https://f/date", c0.to_vec())
        .with("https://f/price", c1.to_vec());
    let refs = vec![
        FactRef::new(ch(c0), "https://f/date"),
        FactRef::new(ch(c1), "https://f/price"),
    ];

    let facts = fetch_and_verify_facts(&fetcher, &refs).await.unwrap();

    // Feed the verified Facts straight into R2's assemble.
    let opts = CategoricalOptions {
        count: 2,
        labels: None,
    };
    let assembled = crate::prompt::assemble("Resolve YES iff BTC > $100k.", &facts, &opts);
    // Both verified contents made it into the prompt.
    assert!(assembled
        .user
        .contains("The date in question is 2025-12-31."));
    assert!(assembled.user.contains("BTC closed at $98,000."));

    // And the assembled request runs through the mock provider.
    let req = crate::prompt::build_request(
        "Resolve YES iff BTC > $100k.",
        &facts,
        opts,
        crate::provider::ModelConfig {
            model_id: "claude-opus-4-8".to_string(),
            provider: "mock".to_string(),
            max_tokens: 1024,
            thinking: Some("adaptive".to_string()),
        },
    );
    let provider = MockProvider::new(1, r#"{"option_index":1}"#, "mock-claude");
    let resp = provider.complete(&req).await.unwrap();
    assert_eq!(resp.option_index, 1);
}
