#![cfg(not(target_arch = "wasm32"))]
mod support;
use futures_util::stream::StreamExt;
use support::*;

#[lunatic::test]
fn test_redirect_301_and_302_and_303_changes_post_to_get() {
    let client = nightfly::Client::new();
    let codes = [301u16, 302, 303];

    for &code in codes.iter() {
        let redirect = server::http(move |req| async move {
            if req.method() == "POST" {
                assert_eq!(req.uri(), &*format!("/{}", code));
                http::Response::builder()
                    .status(code)
                    .header("location", "/dst")
                    .header("server", "test-redirect")
                    .body(Default::default())
                    .unwrap()
            } else {
                assert_eq!(req.method(), "GET");

                http::Response::builder()
                    .header("server", "test-dst")
                    .body(Default::default())
                    .unwrap()
            }
        });

        let url = format!("http://{}/{}", redirect.addr(), code);
        let dst = format!("http://{}/{}", redirect.addr(), "dst");
        let res = client.post(&url).send().unwrap();
        assert_eq!(res.url().as_str(), dst);
        assert_eq!(res.status(), nightfly::StatusCode::OK);
        assert_eq!(
            res.headers().get(nightfly::header::SERVER).unwrap(),
            &"test-dst"
        );
    }
}

#[lunatic::test]
fn test_redirect_307_and_308_tries_to_get_again() {
    let client = nightfly::Client::new();
    let codes = [307u16, 308];
    for &code in codes.iter() {
        let redirect = server::http(move |req| async move {
            assert_eq!(req.method(), "GET");
            if req.uri() == &*format!("/{}", code) {
                http::Response::builder()
                    .status(code)
                    .header("location", "/dst")
                    .header("server", "test-redirect")
                    .body(Default::default())
                    .unwrap()
            } else {
                assert_eq!(req.uri(), "/dst");

                http::Response::builder()
                    .header("server", "test-dst")
                    .body(Default::default())
                    .unwrap()
            }
        });

        let url = format!("http://{}/{}", redirect.addr(), code);
        let dst = format!("http://{}/{}", redirect.addr(), "dst");
        let res = client.get(&url).send().unwrap();
        assert_eq!(res.url().as_str(), dst);
        assert_eq!(res.status(), nightfly::StatusCode::OK);
        assert_eq!(
            res.headers().get(nightfly::header::SERVER).unwrap(),
            &"test-dst"
        );
    }
}

#[lunatic::test]
fn test_redirect_307_and_308_tries_to_post_again() {
    let _ = env_logger::try_init();
    let client = nightfly::Client::new();
    let codes = [307u16, 308];
    for &code in codes.iter() {
        let redirect = server::http(move |mut req| async move {
            assert_eq!(req.method(), "POST");
            assert_eq!(req.headers()["content-length"], "5");

            let data = req.body_mut().next().unwrap().unwrap();
            assert_eq!(&*data, b"Hello");

            if req.uri() == &*format!("/{}", code) {
                http::Response::builder()
                    .status(code)
                    .header("location", "/dst")
                    .header("server", "test-redirect")
                    .body(Default::default())
                    .unwrap()
            } else {
                assert_eq!(req.uri(), "/dst");

                http::Response::builder()
                    .header("server", "test-dst")
                    .body(Default::default())
                    .unwrap()
            }
        });

        let url = format!("http://{}/{}", redirect.addr(), code);
        let dst = format!("http://{}/{}", redirect.addr(), "dst");
        let res = client.post(&url).body("Hello").send().unwrap();
        assert_eq!(res.url().as_str(), dst);
        assert_eq!(res.status(), nightfly::StatusCode::OK);
        assert_eq!(
            res.headers().get(nightfly::header::SERVER).unwrap(),
            &"test-dst"
        );
    }
}

#[cfg(feature = "blocking")]
#[test]
fn test_redirect_307_does_not_try_if_reader_cannot_reset() {
    let client = nightfly::blocking::Client::new();
    let codes = [307u16, 308];
    for &code in codes.iter() {
        let redirect = server::http(move |mut req| async move {
            assert_eq!(req.method(), "POST");
            assert_eq!(req.uri(), &*format!("/{}", code));
            assert_eq!(req.headers()["transfer-encoding"], "chunked");

            let data = req.body_mut().next().unwrap().unwrap();
            assert_eq!(&*data, b"Hello");

            http::Response::builder()
                .status(code)
                .header("location", "/dst")
                .header("server", "test-redirect")
                .body(Default::default())
                .unwrap()
        });

        let url = format!("http://{}/{}", redirect.addr(), code);
        let res = client
            .post(&url)
            .body(nightfly::blocking::Body::new(&b"Hello"[..]))
            .send()
            .unwrap();
        assert_eq!(res.url().as_str(), url);
        assert_eq!(res.status(), code);
    }
}

#[lunatic::test]
fn test_redirect_removes_sensitive_headers() {
    use lunatic::sync::watch;

    let (tx, rx) = watch::channel::<Option<std::net::SocketAddr>>(None);

    let end_server = server::http(move |req| {
        let mut rx = rx.clone();
        async move {
            assert_eq!(req.headers().get("cookie"), None);

            rx.changed().unwrap();
            let mid_addr = rx.borrow().unwrap();
            assert_eq!(
                req.headers()["referer"],
                format!("http://{}/sensitive", mid_addr)
            );
            http::Response::default()
        }
    });

    let end_addr = end_server.addr();

    let mid_server = server::http(move |req| async move {
        assert_eq!(req.headers()["cookie"], "foo=bar");
        http::Response::builder()
            .status(302)
            .header("location", format!("http://{}/end", end_addr))
            .body(Default::default())
            .unwrap()
    });

    tx.send(Some(mid_server.addr())).unwrap();

    nightfly::Client::builder()
        .build()
        .unwrap()
        .get(&format!("http://{}/sensitive", mid_server.addr()))
        .header(
            nightfly::header::COOKIE,
            nightfly::header::HeaderValue::from_static("foo=bar"),
        )
        .send()
        .unwrap();
}

#[lunatic::test]
fn test_redirect_policy_can_return_errors() {
    let server = server::http(move |req| async move {
        assert_eq!(req.uri(), "/loop");
        http::Response::builder()
            .status(302)
            .header("location", "/loop")
            .body(Default::default())
            .unwrap()
    });

    let url = format!("http://{}/loop", server.addr());
    let err = nightfly::get(&url).unwrap_err();
    assert!(err.is_redirect());
}

#[lunatic::test]
fn test_redirect_policy_can_stop_redirects_without_an_error() {
    let server = server::http(move |req| async move {
        assert_eq!(req.uri(), "/no-redirect");
        http::Response::builder()
            .status(302)
            .header("location", "/dont")
            .body(Default::default())
            .unwrap()
    });

    let url = format!("http://{}/no-redirect", server.addr());

    let res = nightfly::Client::builder()
        .redirect(nightfly::redirect::Policy::none())
        .build()
        .unwrap()
        .get(&url)
        .send()
        .unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), nightfly::StatusCode::FOUND);
}

#[lunatic::test]
fn test_referer_is_not_set_if_disabled() {
    let server = server::http(move |req| async move {
        if req.uri() == "/no-refer" {
            http::Response::builder()
                .status(302)
                .header("location", "/dst")
                .body(Default::default())
                .unwrap()
        } else {
            assert_eq!(req.uri(), "/dst");
            assert_eq!(req.headers().get("referer"), None);

            http::Response::default()
        }
    });

    nightfly::Client::builder()
        .referer(false)
        .build()
        .unwrap()
        .get(&format!("http://{}/no-refer", server.addr()))
        .send()
        .unwrap();
}

#[lunatic::test]
fn test_invalid_location_stops_redirect_gh484() {
    let server = server::http(move |_req| async move {
        http::Response::builder()
            .status(302)
            .header("location", "http://www.yikes{KABOOM}")
            .body(Default::default())
            .unwrap()
    });

    let url = format!("http://{}/yikes", server.addr());

    let res = nightfly::get(&url).unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), nightfly::StatusCode::FOUND);
}

#[cfg(feature = "cookies")]
#[lunatic::test]
fn test_redirect_302_with_set_cookies() {
    let code = 302;
    let server = server::http(move |req| async move {
        if req.uri() == "/302" {
            http::Response::builder()
                .status(302)
                .header("location", "/dst")
                .header("set-cookie", "key=value")
                .body(Default::default())
                .unwrap()
        } else {
            assert_eq!(req.uri(), "/dst");
            assert_eq!(req.headers()["cookie"], "key=value");
            http::Response::default()
        }
    });

    let url = format!("http://{}/{}", server.addr(), code);
    let dst = format!("http://{}/{}", server.addr(), "dst");

    let client = nightfly::ClientBuilder::new()
        .cookie_store(true)
        .build()
        .unwrap();
    let res = client.get(&url).send().unwrap();

    assert_eq!(res.url().as_str(), dst);
    assert_eq!(res.status(), nightfly::StatusCode::OK);
}

#[cfg(feature = "__rustls")]
#[lunatic::test]
#[ignore = "Needs TLS support in the test server"]
fn test_redirect_https_only_enforced_gh1312() {
    let server = server::http(move |_req| async move {
        http::Response::builder()
            .status(302)
            .header("location", "http://insecure")
            .body(Default::default())
            .unwrap()
    });

    let url = format!("https://{}/yikes", server.addr());

    let res = nightfly::Client::builder()
        .danger_accept_invalid_certs(true)
        .use_rustls_tls()
        .https_only(true)
        .build()
        .expect("client builder")
        .get(&url)
        .send();

    let err = res.unwrap_err();
    assert!(err.is_redirect());
}
