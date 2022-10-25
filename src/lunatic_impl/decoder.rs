use std::convert::TryFrom;
use std::fmt;
use std::io::{Cursor, Read};

#[cfg(feature = "gzip")]
use async_compression::tokio::bufread::GzipDecoder;

#[cfg(feature = "brotli")]
use async_compression::tokio::bufread::BrotliDecoder;

#[cfg(feature = "deflate")]
use async_compression::tokio::bufread::ZlibDecoder;

use bytes::Bytes;
use http::HeaderMap;

use httparse::{Status, EMPTY_HEADER};
use lunatic::net::TcpStream;
use thiserror::Error;
#[cfg(any(feature = "gzip", feature = "brotli", feature = "deflate"))]
use tokio_util::codec::{BytesCodec, FramedRead};
#[cfg(any(feature = "gzip", feature = "brotli", feature = "deflate"))]
use tokio_util::io::StreamReader;
use url::Url;

use super::super::Body;
use super::http_stream::HttpStream;
use crate::{error, HttpResponse};

#[derive(Clone, Copy, Debug)]
pub(super) struct Accepts {
    #[cfg(feature = "gzip")]
    pub(super) gzip: bool,
    #[cfg(feature = "brotli")]
    pub(super) brotli: bool,
    #[cfg(feature = "deflate")]
    pub(super) deflate: bool,
}

/// A response decompressor over a non-blocking stream of chunks.
///
/// The inner decoder may be constructed asynchronously.
pub(crate) struct Decoder {
    inner: Inner,
}

enum Inner {
    /// A `PlainText` decoder just returns the response content as is.
    PlainText(Vec<u8>),

    /// A `Gzip` decoder will uncompress the gzipped response content before returning it.
    #[cfg(feature = "gzip")]
    Gzip(FramedRead<GzipDecoder<StreamReader<Peekable<IoStream>, Bytes>>, BytesCodec>),

    /// A `Brotli` decoder will uncompress the brotlied response content before returning it.
    #[cfg(feature = "brotli")]
    Brotli(FramedRead<BrotliDecoder<StreamReader<Peekable<IoStream>, Bytes>>, BytesCodec>),

    /// A `Deflate` decoder will uncompress the deflated response content before returning it.
    #[cfg(feature = "deflate")]
    Deflate(FramedRead<ZlibDecoder<StreamReader<Peekable<IoStream>, Bytes>>, BytesCodec>),

    /// A decoder that doesn't have a value yet.
    #[cfg(any(feature = "brotli", feature = "gzip", feature = "deflate"))]
    Pending(Pending),
}

enum DecoderType {
    #[cfg(feature = "gzip")]
    Gzip,
    #[cfg(feature = "brotli")]
    Brotli,
    #[cfg(feature = "deflate")]
    Deflate,
}

impl fmt::Debug for Decoder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Decoder").finish()
    }
}

impl Decoder {
    #[cfg(feature = "blocking")]
    pub(crate) fn empty() -> Decoder {
        Decoder {
            inner: Inner::PlainText(Body::empty().into_stream()),
        }
    }

    /// A plain text decoder.
    ///
    /// This decoder will emit the underlying chunks as-is.
    fn plain_text(body: Vec<u8>) -> Decoder {
        Decoder {
            inner: Inner::PlainText(body),
        }
    }

    /// A gzip decoder.
    ///
    /// This decoder will buffer and decompress chunks that are gzipped.
    #[cfg(feature = "gzip")]
    fn gzip(body: Body) -> Decoder {
        use futures_util::StreamExt;

        Decoder {
            inner: Inner::Pending(Pending(
                IoStream(body.into_stream()).peekable(),
                DecoderType::Gzip,
            )),
        }
    }

    /// A brotli decoder.
    ///
    /// This decoder will buffer and decompress chunks that are brotlied.
    #[cfg(feature = "brotli")]
    fn brotli(body: Body) -> Decoder {
        use futures_util::StreamExt;

        Decoder {
            inner: Inner::Pending(Pending(
                IoStream(body.into_stream()).peekable(),
                DecoderType::Brotli,
            )),
        }
    }

    /// A deflate decoder.
    ///
    /// This decoder will buffer and decompress chunks that are deflated.
    #[cfg(feature = "deflate")]
    fn deflate(body: Body) -> Decoder {
        use futures_util::StreamExt;

        Decoder {
            inner: Inner::Pending(Pending(
                IoStream(body.into_stream()).peekable(),
                DecoderType::Deflate,
            )),
        }
    }

    pub fn decode(&self) -> Vec<u8> {
        match &self.inner {
            Inner::PlainText(text) => text.clone(),
        }
    }

    #[cfg(any(feature = "brotli", feature = "gzip", feature = "deflate"))]
    fn detect_encoding(headers: &mut HeaderMap, encoding_str: &str) -> bool {
        use http::header::{CONTENT_ENCODING, CONTENT_LENGTH, TRANSFER_ENCODING};
        use lunatic_log::warn;

        let mut is_content_encoded = {
            headers
                .get_all(CONTENT_ENCODING)
                .iter()
                .any(|enc| enc == encoding_str)
                || headers
                    .get_all(TRANSFER_ENCODING)
                    .iter()
                    .any(|enc| enc == encoding_str)
        };
        if is_content_encoded {
            if let Some(content_length) = headers.get(CONTENT_LENGTH) {
                if content_length == "0" {
                    warn!("{} response with content-length of 0", encoding_str);
                    is_content_encoded = false;
                }
            }
        }
        if is_content_encoded {
            headers.remove(CONTENT_ENCODING);
            headers.remove(CONTENT_LENGTH);
        }
        is_content_encoded
    }

    /// Constructs a Decoder from a hyper request.
    ///
    /// A decoder is just a wrapper around the hyper request that knows
    /// how to decode the content body of the request.
    ///
    /// Uses the correct variant by inspecting the Content-Encoding header.
    pub(super) fn detect(_headers: &mut HeaderMap, body: Vec<u8>, _accepts: Accepts) -> Decoder {
        #[cfg(feature = "gzip")]
        {
            if _accepts.gzip && Decoder::detect_encoding(_headers, "gzip") {
                return Decoder::gzip(body);
            }
        }

        #[cfg(feature = "brotli")]
        {
            if _accepts.brotli && Decoder::detect_encoding(_headers, "br") {
                return Decoder::brotli(body);
            }
        }

        #[cfg(feature = "deflate")]
        {
            if _accepts.deflate && Decoder::detect_encoding(_headers, "deflate") {
                return Decoder::deflate(body);
            }
        }

        Decoder::plain_text(body)
    }
}

impl Read for Decoder {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Do a read or poll for a pending decoder value.
        match self.inner {
            #[cfg(any(feature = "brotli", feature = "gzip", feature = "deflate"))]
            Inner::Pending(ref mut future) => match Pin::new(future).poll(cx) {
                Poll::Ready(Ok(inner)) => {
                    self.inner = inner;
                    return self.poll_next(cx);
                }
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Some(Err(crate::error::decode_io(e))));
                }
                Poll::Pending => return Poll::Pending,
            },
            Inner::PlainText(ref mut body) => Cursor::new(body).read(buf),
            #[cfg(feature = "gzip")]
            Inner::Gzip(ref mut decoder) => {
                return match futures_core::ready!(Pin::new(decoder).poll_next(cx)) {
                    Some(Ok(bytes)) => Poll::Ready(Some(Ok(bytes.freeze()))),
                    Some(Err(err)) => Poll::Ready(Some(Err(crate::error::decode_io(err)))),
                    None => Poll::Ready(None),
                };
            }
            #[cfg(feature = "brotli")]
            Inner::Brotli(ref mut decoder) => {
                return match futures_core::ready!(Pin::new(decoder).poll_next(cx)) {
                    Some(Ok(bytes)) => Poll::Ready(Some(Ok(bytes.freeze()))),
                    Some(Err(err)) => Poll::Ready(Some(Err(crate::error::decode_io(err)))),
                    None => Poll::Ready(None),
                };
            }
            #[cfg(feature = "deflate")]
            Inner::Deflate(ref mut decoder) => {
                return match futures_core::ready!(Pin::new(decoder).poll_next(cx)) {
                    Some(Ok(bytes)) => Poll::Ready(Some(Ok(bytes.freeze()))),
                    Some(Err(err)) => Poll::Ready(Some(Err(crate::error::decode_io(err)))),
                    None => Poll::Ready(None),
                };
            }
        }
    }
}

// impl HttpBody for Decoder {
//     type Data = Bytes;
//     type Error = crate::Error;

//     fn poll_data(
//         self: Pin<&mut Self>,
//         cx: &mut Context,
//     ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
//         self.poll_next(cx)
//     }

//     fn poll_trailers(
//         self: Pin<&mut Self>,
//         _cx: &mut Context,
//     ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
//         Poll::Ready(Ok(None))
//     }

//     fn size_hint(&self) -> http_body::SizeHint {
//         match self.inner {
//             Inner::PlainText(ref body) => HttpBody::size_hint(body),
//             // the rest are "unknown", so default
//             #[cfg(any(feature = "brotli", feature = "gzip", feature = "deflate"))]
//             _ => http_body::SizeHint::default(),
//         }
//     }
// }

// impl Future for Pending {
//     type Output = Result<Inner, std::io::Error>;

//     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         use futures_util::StreamExt;

//         match futures_core::ready!(Pin::new(&mut self.0).poll_peek(cx)) {
//             Some(Ok(_)) => {
//                 // fallthrough
//             }
//             Some(Err(_e)) => {
//                 // error was just a ref, so we need to really poll to move it
//                 return Poll::Ready(Err(futures_core::ready!(
//                     Pin::new(&mut self.0).poll_next(cx)
//                 )
//                 .expect("just peeked Some")
//                 .unwrap_err()));
//             }
//             None => return Poll::Ready(Ok(Inner::PlainText(Body::empty().into_stream()))),
//         };

//         let _body = std::mem::replace(
//             &mut self.0,
//             IoStream(Body::empty().into_stream()).peekable(),
//         );

//         match self.1 {
//             #[cfg(feature = "brotli")]
//             DecoderType::Brotli => Poll::Ready(Ok(Inner::Brotli(FramedRead::new(
//                 BrotliDecoder::new(StreamReader::new(_body)),
//                 BytesCodec::new(),
//             )))),
//             #[cfg(feature = "gzip")]
//             DecoderType::Gzip => Poll::Ready(Ok(Inner::Gzip(FramedRead::new(
//                 GzipDecoder::new(StreamReader::new(_body)),
//                 BytesCodec::new(),
//             )))),
//             #[cfg(feature = "deflate")]
//             DecoderType::Deflate => Poll::Ready(Ok(Inner::Deflate(FramedRead::new(
//                 ZlibDecoder::new(StreamReader::new(_body)),
//                 BytesCodec::new(),
//             )))),
//         }
//     }
// }

const MAX_REQUEST_SIZE: usize = 10 * 1024 * 1024;
const REQUEST_BUFFER_SIZE: usize = 4096;
const MAX_HEADERS: usize = 128;

/// The result of parsing a response from a buffer.
type ResponseResult = Result<HttpResponse, ParseResponseError>;

#[derive(Debug, Error)]
pub(crate) enum ParseResponseError {
    #[error("tcp stream closed")]
    TcpStreamClosed,
    #[error("tcp stream closed without data")]
    TcpStreamClosedWithoutData,
    #[error("http parse error: {0}")]
    HttpParseError(httparse::Error),
    #[error("response too large")]
    ResponseTooLarge,
    #[error("unknown code")]
    UnknownCode,
    #[error("invalid chunk size")]
    InvalidChunkSize,
    #[error("invalid chunk separator")]
    MissingChunkSeparator,
}

pub(crate) fn parse_response(
    mut response_buffer: Vec<u8>,
    mut stream: HttpStream,
    url: Url,
) -> ResponseResult {
    let mut buffer = [0_u8; REQUEST_BUFFER_SIZE];
    let mut headers = [EMPTY_HEADER; MAX_HEADERS];

    // Loop until at least one complete response is read.
    let (response_raw, offset) = loop {
        // In case of pipelined responses the `response_buffer` is going to come
        // prefilled with some data, and we should attempt to parse it into a response
        // before we decide to read more from `TcpStream`.
        let mut response_raw = httparse::Response::new(&mut headers);
        match response_raw.parse(&response_buffer) {
            Ok(state) => match state {
                Status::Complete(offset) => {
                    // Continue outside the loop.
                    break (response_raw, offset);
                }
                Status::Partial => {
                    // Read more data from TCP stream
                    let n = stream.read(&mut buffer);
                    if n.is_err() || *n.as_ref().unwrap() == 0 {
                        if response_buffer.is_empty() {
                            return Err(ParseResponseError::TcpStreamClosedWithoutData);
                        } else {
                            return Err(ParseResponseError::TcpStreamClosed);
                        }
                    }
                    let n = n.unwrap();
                    // Invalidate references in `headers` that could point to the previous
                    // `response_buffer` before extending it.
                    headers = [EMPTY_HEADER; MAX_HEADERS];
                    response_buffer.extend(&buffer[..n]);
                    // If response passed max size, abort
                    if response_buffer.len() > MAX_REQUEST_SIZE {
                        return Err(ParseResponseError::ResponseTooLarge);
                    }
                }
            },
            Err(err) => {
                return Err(ParseResponseError::HttpParseError(err));
            }
        }
    };

    lunatic_log::debug!(
        "Received RAW Response {:?}",
        String::from_utf8(response_buffer.clone())
    );

    // At this point one full response header is available, but the body (if it
    // exists) might not be fully loaded yet.

    let status_code = match http::StatusCode::try_from(response_raw.code.unwrap()) {
        Ok(code) => code,
        Err(_) => {
            return Err(ParseResponseError::UnknownCode);
        }
    };
    let response = http::Response::builder().status(status_code);
    let response = response_raw
        .headers
        .iter()
        .fold(response, |response, header| {
            response.header(header.name, header.value)
        });
    let content_length = response
        .headers_ref()
        .and_then(|headers| headers.get("content-length"))
        .and_then(|content_length| {
            std::str::from_utf8(content_length.as_bytes())
                .ok()?
                .parse::<usize>()
                .ok()
        });
    let chunked = response
        .headers_ref()
        .and_then(|headers| headers.get("transfer-encoding"))
        .map(|transfer_encoding| transfer_encoding.as_bytes() == b"chunked")
        .unwrap_or(false);
    // If content-length exists, response has a body
    let res = response.body(vec![0u8; 0]).unwrap();
    let mut res = HttpResponse {
        headers: res.headers().to_owned(),
        status: res.status().to_owned(),
        version: res.version().to_owned(),
        body: vec![],
        url,
    };
    if chunked {
        let mut chunk_offset = offset;
        let mut body = Vec::new();
        loop {
            let chunk = httparse::parse_chunk_size(&response_buffer[chunk_offset..]);
            match chunk {
                Ok(Status::Complete((idx, size))) => {
                    if size == 0 && response_buffer[chunk_offset + idx..].starts_with(b"\r\n") {
                        res.body = body;
                        return Ok(res);
                    }

                    let missing_bytes = (size as usize)
                        .saturating_sub(response_buffer.len() - idx - chunk_offset - 2);
                    if missing_bytes > 0 {
                        let mut buf = vec![0u8; missing_bytes];
                        let n = stream.read(&mut buf).unwrap();
                        response_buffer.extend(&buf[..n]);
                        continue;
                    }

                    let chunk =
                        &response_buffer[chunk_offset + idx..chunk_offset + idx + size as usize];
                    chunk_offset += idx + size as usize + 2;
                    body.extend(chunk);
                }
                Ok(Status::Partial) => {
                    let mut buf = vec![0u8; REQUEST_BUFFER_SIZE];
                    let n = stream.read(&mut buf).unwrap();
                    response_buffer.extend(&buf[..n]);
                }
                Err(_) => {
                    return Err(ParseResponseError::InvalidChunkSize);
                }
            }
        }
    } else {
        match content_length {
            Some(content_length) => {
                if response_buffer[offset..].len() == content_length {
                    // Complete content is captured from the response w/o trailing pipelined
                    // responses.
                    res.body = response_buffer[offset..].to_owned();
                    Ok(res)
                } else {
                    // Read the rest from TCP stream to form a full response
                    let rest = content_length - response_buffer[offset..].len();
                    let mut buffer = vec![0u8; rest];
                    stream.read_exact(&mut buffer).unwrap();
                    response_buffer.extend(&buffer);
                    res.body = response_buffer[offset..].to_owned();
                    Ok(res)
                }
            }
            None => Ok(res),
        }
    }
}

// ===== impl Accepts =====

impl Accepts {
    pub(super) fn none() -> Self {
        Accepts {
            #[cfg(feature = "gzip")]
            gzip: false,
            #[cfg(feature = "brotli")]
            brotli: false,
            #[cfg(feature = "deflate")]
            deflate: false,
        }
    }

    pub(super) fn as_str(&self) -> Option<&'static str> {
        match (self.is_gzip(), self.is_brotli(), self.is_deflate()) {
            (true, true, true) => Some("gzip, br, deflate"),
            (true, true, false) => Some("gzip, br"),
            (true, false, true) => Some("gzip, deflate"),
            (false, true, true) => Some("br, deflate"),
            (true, false, false) => Some("gzip"),
            (false, true, false) => Some("br"),
            (false, false, true) => Some("deflate"),
            (false, false, false) => None,
        }
    }

    fn is_gzip(&self) -> bool {
        #[cfg(feature = "gzip")]
        {
            self.gzip
        }

        #[cfg(not(feature = "gzip"))]
        {
            false
        }
    }

    fn is_brotli(&self) -> bool {
        #[cfg(feature = "brotli")]
        {
            self.brotli
        }

        #[cfg(not(feature = "brotli"))]
        {
            false
        }
    }

    fn is_deflate(&self) -> bool {
        #[cfg(feature = "deflate")]
        {
            self.deflate
        }

        #[cfg(not(feature = "deflate"))]
        {
            false
        }
    }
}

impl Default for Accepts {
    fn default() -> Accepts {
        Accepts {
            #[cfg(feature = "gzip")]
            gzip: true,
            #[cfg(feature = "brotli")]
            brotli: true,
            #[cfg(feature = "deflate")]
            deflate: true,
        }
    }
}
