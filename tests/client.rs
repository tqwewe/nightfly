#![cfg(not(target_arch = "wasm32"))]
mod support;
use futures_util::stream::StreamExt;
use support::*;

use nightfly::Client;

#[lunatic::test]
fn auto_headers() {
    let server = server::http(move |req| async move {
        assert_eq!(req.method(), "GET");

        assert_eq!(req.headers()["accept"], "*/*");
        assert_eq!(req.headers().get("user-agent"), None);
        if cfg!(feature = "gzip") {
            assert!(req.headers()["accept-encoding"]
                .to_str()
                .unwrap()
                .contains("gzip"));
        }
        if cfg!(feature = "brotli") {
            assert!(req.headers()["accept-encoding"]
                .to_str()
                .unwrap()
                .contains("br"));
        }
        if cfg!(feature = "deflate") {
            assert!(req.headers()["accept-encoding"]
                .to_str()
                .unwrap()
                .contains("deflate"));
        }

        http::Response::default()
    });

    let url = format!("http://{}/1", server.addr());
    let res = nightfly::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(&url)
        .send()
        .unwrap();

    assert_eq!(res.url().as_str(), &url);
    assert_eq!(res.status(), nightfly::StatusCode::OK);
    assert_eq!(res.remote_addr(), Some(server.addr()));
}

#[lunatic::test]
fn user_agent() {
    let server = server::http(move |req| async move {
        assert_eq!(req.headers()["user-agent"], "nightfly-test-agent");
        http::Response::default()
    });

    let url = format!("http://{}/ua", server.addr());
    let res = nightfly::Client::builder()
        .user_agent("nightfly-test-agent")
        .build()
        .expect("client builder")
        .get(&url)
        .send()
        .expect("request");

    assert_eq!(res.status(), nightfly::StatusCode::OK);
}

#[lunatic::test]
fn response_text() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let client = Client::new();

    let res = client
        .get(&format!("http://{}/text", server.addr()))
        .send()
        .expect("Failed to get");
    assert_eq!(res.content_length(), Some(5));
    let text = res.text().expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[lunatic::test]
fn response_bytes() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let client = Client::new();

    let res = client
        .get(&format!("http://{}/bytes", server.addr()))
        .send()
        .expect("Failed to get");
    assert_eq!(res.content_length(), Some(5));
    let bytes = res.bytes().expect("res.bytes()");
    assert_eq!("Hello", bytes);
}

#[lunatic::test]
#[cfg(feature = "json")]
fn response_json() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| async { http::Response::new("\"Hello\"".into()) });

    let client = Client::new();

    let res = client
        .get(&format!("http://{}/json", server.addr()))
        .send()
        .expect("Failed to get");
    let text = res.json::<String>().expect("Failed to get json");
    assert_eq!("Hello", text);
}

#[lunatic::test]
fn body_pipe_response() {
    let _ = env_logger::try_init();

    let server = server::http(move |mut req| async move {
        if req.uri() == "/get" {
            http::Response::new("pipe me".into())
        } else {
            assert_eq!(req.uri(), "/pipe");
            assert_eq!(req.headers()["transfer-encoding"], "chunked");

            let mut full: Vec<u8> = Vec::new();
            while let Some(item) = req.body_mut().next() {
                full.extend(&*item.unwrap());
            }

            assert_eq!(full, b"pipe me");

            http::Response::default()
        }
    });

    let client = Client::new();

    let res1 = client
        .get(&format!("http://{}/get", server.addr()))
        .send()
        .expect("get1");

    assert_eq!(res1.status(), nightfly::StatusCode::OK);
    assert_eq!(res1.content_length(), Some(7));

    // and now ensure we can "pipe" the response to another request
    let res2 = client
        .post(&format!("http://{}/pipe", server.addr()))
        .body(res1)
        .send()
        .expect("res2");

    assert_eq!(res2.status(), nightfly::StatusCode::OK);
}

#[lunatic::test]
fn overridden_dns_resolution_with_gai() {
    let _ = env_logger::builder().is_test(true).try_init();
    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let overridden_domain = "rust-lang.org";
    let url = format!(
        "http://{}:{}/domain_override",
        overridden_domain,
        server.addr().port()
    );
    let client = nightfly::Client::builder()
        .resolve(overridden_domain, server.addr())
        .build()
        .expect("client builder");
    let req = client.get(&url);
    let res = req.send().expect("request");

    assert_eq!(res.status(), nightfly::StatusCode::OK);
    let text = res.text().expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[lunatic::test]
fn overridden_dns_resolution_with_gai_multiple() {
    let _ = env_logger::builder().is_test(true).try_init();
    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let overridden_domain = "rust-lang.org";
    let url = format!(
        "http://{}:{}/domain_override",
        overridden_domain,
        server.addr().port()
    );
    // the server runs on IPv4 localhost, so provide both IPv4 and IPv6 and let the happy eyeballs
    // algorithm decide which address to use.
    let client = nightfly::Client::builder()
        .resolve_to_addrs(
            overridden_domain,
            &[
                std::net::SocketAddr::new(
                    std::net::IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                    server.addr().port(),
                ),
                server.addr(),
            ],
        )
        .build()
        .expect("client builder");
    let req = client.get(&url);
    let res = req.send().expect("request");

    assert_eq!(res.status(), nightfly::StatusCode::OK);
    let text = res.text().expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[cfg(feature = "trust-dns")]
#[lunatic::test]
fn overridden_dns_resolution_with_trust_dns() {
    let _ = env_logger::builder().is_test(true).try_init();
    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let overridden_domain = "rust-lang.org";
    let url = format!(
        "http://{}:{}/domain_override",
        overridden_domain,
        server.addr().port()
    );
    let client = nightfly::Client::builder()
        .resolve(overridden_domain, server.addr())
        .trust_dns(true)
        .build()
        .expect("client builder");
    let req = client.get(&url);
    let res = req.send().expect("request");

    assert_eq!(res.status(), nightfly::StatusCode::OK);
    let text = res.text().expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[cfg(feature = "trust-dns")]
#[lunatic::test]
fn overridden_dns_resolution_with_trust_dns_multiple() {
    let _ = env_logger::builder().is_test(true).try_init();
    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let overridden_domain = "rust-lang.org";
    let url = format!(
        "http://{}:{}/domain_override",
        overridden_domain,
        server.addr().port()
    );
    // the server runs on IPv4 localhost, so provide both IPv4 and IPv6 and let the happy eyeballs
    // algorithm decide which address to use.
    let client = nightfly::Client::builder()
        .resolve_to_addrs(
            overridden_domain,
            &[
                std::net::SocketAddr::new(
                    std::net::IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                    server.addr().port(),
                ),
                server.addr(),
            ],
        )
        .trust_dns(true)
        .build()
        .expect("client builder");
    let req = client.get(&url);
    let res = req.send().expect("request");

    assert_eq!(res.status(), nightfly::StatusCode::OK);
    let text = res.text().expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[cfg(any(feature = "native-tls", feature = "__rustls",))]
#[test]
fn use_preconfigured_tls_with_bogus_backend() {
    struct DefinitelyNotTls;

    nightfly::Client::builder()
        .use_preconfigured_tls(DefinitelyNotTls)
        .build()
        .expect_err("definitely is not TLS");
}

#[cfg(feature = "native-tls")]
#[test]
fn use_preconfigured_native_tls_default() {
    extern crate native_tls_crate;

    let tls = native_tls_crate::TlsConnector::builder()
        .build()
        .expect("tls builder");

    nightfly::Client::builder()
        .use_preconfigured_tls(tls)
        .build()
        .expect("preconfigured default tls");
}

#[cfg(feature = "__rustls")]
#[test]
fn use_preconfigured_rustls_default() {
    extern crate rustls;

    let root_cert_store = rustls::RootCertStore::empty();
    let tls = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();

    nightfly::Client::builder()
        .use_preconfigured_tls(tls)
        .build()
        .expect("preconfigured rustls tls");
}

#[cfg(feature = "__rustls")]
#[lunatic::test]
#[ignore = "Needs TLS support in the test server"]
fn http2_upgrade() {
    let server = server::http(move |_| async move { http::Response::default() });

    let url = format!("https://localhost:{}", server.addr().port());
    let res = nightfly::Client::builder()
        .danger_accept_invalid_certs(true)
        .use_rustls_tls()
        .build()
        .expect("client builder")
        .get(&url)
        .send()
        .expect("request");

    assert_eq!(res.status(), nightfly::StatusCode::OK);
    assert_eq!(res.version(), nightfly::Version::HTTP_2);
}

#[cfg(feature = "default-tls")]
#[lunatic::test]
fn test_allowed_methods() {
    let resp = nightfly::Client::builder()
        .https_only(true)
        .build()
        .expect("client builder")
        .get("https://google.com")
        .send();

    assert!(resp.is_ok());

    let resp = nightfly::Client::builder()
        .https_only(true)
        .build()
        .expect("client builder")
        .get("http://google.com")
        .send();

    assert!(resp.is_err());
}
