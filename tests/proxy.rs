#![cfg(not(target_arch = "wasm32"))]
mod support;
use support::*;

use std::env;

#[lunatic::test]
fn http_proxy() {
    let url = "http://hyper.rs/prox";
    let server = server::http(move |req| {
        assert_eq!(req.method(), "GET");
        assert_eq!(req.uri(), url);
        assert_eq!(req.headers()["host"], "hyper.rs");

        async { http::Response::default() }
    });

    let proxy = format!("http://{}", server.addr());

    let res = nightfly::Client::builder()
        .proxy(nightfly::Proxy::http(&proxy).unwrap())
        .build()
        .unwrap()
        .get(url)
        .send()
        .unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), nightfly::StatusCode::OK);
}

#[lunatic::test]
fn http_proxy_basic_auth() {
    let url = "http://hyper.rs/prox";
    let server = server::http(move |req| {
        assert_eq!(req.method(), "GET");
        assert_eq!(req.uri(), url);
        assert_eq!(req.headers()["host"], "hyper.rs");
        assert_eq!(
            req.headers()["proxy-authorization"],
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );

        async { http::Response::default() }
    });

    let proxy = format!("http://{}", server.addr());

    let res = nightfly::Client::builder()
        .proxy(
            nightfly::Proxy::http(&proxy)
                .unwrap()
                .basic_auth("Aladdin", "open sesame"),
        )
        .build()
        .unwrap()
        .get(url)
        .send()
        .unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), nightfly::StatusCode::OK);
}

#[lunatic::test]
fn http_proxy_basic_auth_parsed() {
    let url = "http://hyper.rs/prox";
    let server = server::http(move |req| {
        assert_eq!(req.method(), "GET");
        assert_eq!(req.uri(), url);
        assert_eq!(req.headers()["host"], "hyper.rs");
        assert_eq!(
            req.headers()["proxy-authorization"],
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );

        async { http::Response::default() }
    });

    let proxy = format!("http://Aladdin:open sesame@{}", server.addr());

    let res = nightfly::Client::builder()
        .proxy(nightfly::Proxy::http(&proxy).unwrap())
        .build()
        .unwrap()
        .get(url)
        .send()
        .unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), nightfly::StatusCode::OK);
}

#[lunatic::test]
fn system_http_proxy_basic_auth_parsed() {
    let url = "http://hyper.rs/prox";
    let server = server::http(move |req| {
        assert_eq!(req.method(), "GET");
        assert_eq!(req.uri(), url);
        assert_eq!(req.headers()["host"], "hyper.rs");
        assert_eq!(
            req.headers()["proxy-authorization"],
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );

        async { http::Response::default() }
    });

    // save system setting first.
    let system_proxy = env::var("http_proxy");

    // set-up http proxy.
    env::set_var(
        "http_proxy",
        format!("http://Aladdin:open sesame@{}", server.addr()),
    );

    let res = nightfly::Client::builder()
        .build()
        .unwrap()
        .get(url)
        .send()
        .unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), nightfly::StatusCode::OK);

    // reset user setting.
    match system_proxy {
        Err(_) => env::remove_var("http_proxy"),
        Ok(proxy) => env::set_var("http_proxy", proxy),
    }
}

#[lunatic::test]
fn test_no_proxy() {
    let server = server::http(move |req| {
        assert_eq!(req.method(), "GET");
        assert_eq!(req.uri(), "/4");

        async { http::Response::default() }
    });
    let proxy = format!("http://{}", server.addr());
    let url = format!("http://{}/4", server.addr());

    // set up proxy and use no_proxy to clear up client builder proxies.
    let res = nightfly::Client::builder()
        .proxy(nightfly::Proxy::http(&proxy).unwrap())
        .no_proxy()
        .build()
        .unwrap()
        .get(&url)
        .send()
        .unwrap();

    assert_eq!(res.url().as_str(), &url);
    assert_eq!(res.status(), nightfly::StatusCode::OK);
}

#[cfg_attr(not(feature = "__internal_proxy_sys_no_cache"), ignore)]
#[lunatic::test]
fn test_using_system_proxy() {
    let url = "http://not.a.real.sub.hyper.rs/prox";
    let server = server::http(move |req| {
        assert_eq!(req.method(), "GET");
        assert_eq!(req.uri(), url);
        assert_eq!(req.headers()["host"], "not.a.real.sub.hyper.rs");

        async { http::Response::default() }
    });

    // Note: we're relying on the `__internal_proxy_sys_no_cache` feature to
    // check the environment every time.

    // save system setting first.
    let system_proxy = env::var("http_proxy");
    // set-up http proxy.
    env::set_var("http_proxy", format!("http://{}", server.addr()));

    // system proxy is used by default
    let res = nightfly::get(url).unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), nightfly::StatusCode::OK);

    // reset user setting.
    match system_proxy {
        Err(_) => env::remove_var("http_proxy"),
        Ok(proxy) => env::set_var("http_proxy", proxy),
    }
}

#[lunatic::test]
fn http_over_http() {
    let url = "http://hyper.rs/prox";

    let server = server::http(move |req| {
        assert_eq!(req.method(), "GET");
        assert_eq!(req.uri(), url);
        assert_eq!(req.headers()["host"], "hyper.rs");

        async { http::Response::default() }
    });

    let proxy = format!("http://{}", server.addr());

    let res = nightfly::Client::builder()
        .proxy(nightfly::Proxy::http(&proxy).unwrap())
        .build()
        .unwrap()
        .get(url)
        .send()
        .unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), nightfly::StatusCode::OK);
}
