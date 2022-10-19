mod support;
use std::io::Write;
use support::*;

#[lunatic::test]
fn deflate_response() {
    deflate_case(10_000, 4096);
}

#[lunatic::test]
fn deflate_single_byte_chunks() {
    deflate_case(10, 1);
}

#[lunatic::test]
fn test_deflate_empty_body() {
    let server = server::http(move |req| async move {
        assert_eq!(req.method(), "HEAD");

        http::Response::builder()
            .header("content-encoding", "deflate")
            .header("content-length", 100)
            .body(Default::default())
            .unwrap()
    });

    let client = nightfly::Client::new();
    let res = client
        .head(&format!("http://{}/deflate", server.addr()))
        .send()
        .unwrap();

    let body = res.text().unwrap();

    assert_eq!(body, "");
}

#[lunatic::test]
fn test_accept_header_is_not_changed_if_set() {
    let server = server::http(move |req| async move {
        assert_eq!(req.headers()["accept"], "application/json");
        assert!(req.headers()["accept-encoding"]
            .to_str()
            .unwrap()
            .contains("deflate"));
        http::Response::default()
    });

    let client = nightfly::Client::new();

    let res = client
        .get(&format!("http://{}/accept", server.addr()))
        .header(
            nightfly::header::ACCEPT,
            nightfly::header::HeaderValue::from_static("application/json"),
        )
        .send()
        .unwrap();

    assert_eq!(res.status(), nightfly::StatusCode::OK);
}

#[lunatic::test]
fn test_accept_encoding_header_is_not_changed_if_set() {
    let server = server::http(move |req| async move {
        assert_eq!(req.headers()["accept"], "*/*");
        assert_eq!(req.headers()["accept-encoding"], "identity");
        http::Response::default()
    });

    let client = nightfly::Client::new();

    let res = client
        .get(&format!("http://{}/accept-encoding", server.addr()))
        .header(
            nightfly::header::ACCEPT_ENCODING,
            nightfly::header::HeaderValue::from_static("identity"),
        )
        .send()
        .unwrap();

    assert_eq!(res.status(), nightfly::StatusCode::OK);
}

fn deflate_case(response_size: usize, chunk_size: usize) {
    use futures_util::stream::StreamExt;

    let content: String = (0..response_size)
        .into_iter()
        .map(|i| format!("test {}", i))
        .collect();
    let mut encoder = libflate::zlib::Encoder::new(Vec::new()).unwrap();
    match encoder.write(content.as_bytes()) {
        Ok(n) => assert!(n > 0, "Failed to write to encoder."),
        _ => panic!("Failed to deflate encode string."),
    };

    let deflated_content = encoder.finish().into_result().unwrap();

    let mut response = format!(
        "\
         HTTP/1.1 200 OK\r\n\
         Server: test-accept\r\n\
         Content-Encoding: deflate\r\n\
         Content-Length: {}\r\n\
         \r\n",
        &deflated_content.len()
    )
    .into_bytes();
    response.extend(&deflated_content);

    let server = server::http(move |req| {
        assert!(req.headers()["accept-encoding"]
            .to_str()
            .unwrap()
            .contains("deflate"));

        let deflated = deflated_content.clone();
        async move {
            let len = deflated.len();
            let stream =
                futures_util::stream::unfold((deflated, 0), move |(deflated, pos)| async move {
                    let chunk = deflated.chunks(chunk_size).nth(pos)?.to_vec();

                    Some((chunk, (deflated, pos + 1)))
                });

            let body = hyper::Body::wrap_stream(stream.map(Ok::<_, std::convert::Infallible>));

            http::Response::builder()
                .header("content-encoding", "deflate")
                .header("content-length", len)
                .body(body)
                .unwrap()
        }
    });

    let client = nightfly::Client::new();

    let res = client
        .get(&format!("http://{}/deflate", server.addr()))
        .send()
        .expect("response");

    let body = res.text().expect("text");
    assert_eq!(body, content);
}
