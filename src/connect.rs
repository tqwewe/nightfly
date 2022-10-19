#[cfg(feature = "__tls")]
use http::header::HeaderValue;
use http::uri::{Authority, Scheme};
use http::Uri;
use lunatic::net::TcpStream;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "trust-dns")]
use crate::dns::TrustDnsResolver;
use crate::proxy::Proxy;

#[derive(Clone)]
pub(crate) enum HttpConnector {
    Gai,
    GaiWithDnsOverrides(DnsResolverWithOverrides),
    #[cfg(feature = "trust-dns")]
    TrustDns(hyper::client::HttpConnector<TrustDnsResolver>),
    #[cfg(feature = "trust-dns")]
    TrustDnsWithOverrides(hyper::client::HttpConnector<DnsResolverWithOverrides<TrustDnsResolver>>),
}

impl HttpConnector {
    pub(crate) fn new_gai() -> Self {
        Self::Gai
    }

    pub(crate) fn new_gai_with_overrides(overrides: HashMap<String, Vec<SocketAddr>>) -> Self {
        let overridden_resolver = DnsResolverWithOverrides::new(overrides);
        Self::GaiWithDnsOverrides(overridden_resolver)
    }

    // pub fn set_keepalive(&mut self, timeout) {
    //     match self {
    //         Htt
    //     }
    // }

    #[cfg(feature = "trust-dns")]
    pub(crate) fn new_trust_dns() -> crate::Result<HttpConnector> {
        TrustDnsResolver::new()
            .map(hyper::client::HttpConnector::new_with_resolver)
            .map(Self::TrustDns)
            .map_err(crate::error::builder)
    }

    #[cfg(feature = "trust-dns")]
    pub(crate) fn new_trust_dns_with_overrides(
        overrides: HashMap<String, Vec<SocketAddr>>,
    ) -> crate::Result<HttpConnector> {
        TrustDnsResolver::new()
            .map(|resolver| DnsResolverWithOverrides::new(resolver, overrides))
            .map(hyper::client::HttpConnector::new_with_resolver)
            .map(Self::TrustDnsWithOverrides)
            .map_err(crate::error::builder)
    }
}

#[derive(Clone)]
pub(crate) struct Connector {
    inner: Inner,
    proxies: Arc<Vec<Proxy>>,
    // verbose: verbose::Wrapper,
    timeout: Option<Duration>,
    #[cfg(feature = "__tls")]
    nodelay: bool,
    #[cfg(feature = "__tls")]
    user_agent: Option<HeaderValue>,
    keep_alive: Option<Duration>,
}

#[derive(Clone)]
enum Inner {
    #[cfg(not(feature = "__tls"))]
    Http(HttpConnector),
    #[cfg(feature = "default-tls")]
    DefaultTls(HttpConnector, TlsConnector),
    #[cfg(feature = "__rustls")]
    RustlsTls {
        http: HttpConnector,
        tls: Arc<rustls::ClientConfig>,
        tls_proxy: Arc<rustls::ClientConfig>,
    },
}

impl Connector {
    #[cfg(not(feature = "__tls"))]
    pub(crate) fn new<T>(
        mut http: HttpConnector,
        proxies: Arc<Vec<Proxy>>,
        local_addr: T,
        nodelay: bool,
    ) -> Connector
    where
        T: Into<Option<IpAddr>>,
    {
        // http.set_local_address(local_addr.into());
        // http.set_nodelay(nodelay);
        Connector {
            inner: Inner::Http(http),
            // verbose: verbose::OFF,
            proxies,
            timeout: None,
            keep_alive: None,
        }
    }

    #[cfg(feature = "default-tls")]
    pub(crate) fn new_default_tls<T>(
        http: HttpConnector,
        tls: TlsConnectorBuilder,
        proxies: Arc<Vec<Proxy>>,
        user_agent: Option<HeaderValue>,
        local_addr: T,
        nodelay: bool,
    ) -> crate::Result<Connector>
    where
        T: Into<Option<IpAddr>>,
    {
        let tls = tls.build().map_err(crate::error::builder)?;
        Ok(Self::from_built_default_tls(
            http, tls, proxies, user_agent, local_addr, nodelay,
        ))
    }

    #[cfg(feature = "default-tls")]
    pub(crate) fn from_built_default_tls<T>(
        mut http: HttpConnector,
        tls: TlsConnector,
        proxies: Arc<Vec<Proxy>>,
        user_agent: Option<HeaderValue>,
        local_addr: T,
        nodelay: bool,
    ) -> Connector
    where
        T: Into<Option<IpAddr>>,
    {
        http.set_local_address(local_addr.into());
        http.enforce_http(false);

        Connector {
            inner: Inner::DefaultTls(http, tls),
            proxies,
            verbose: verbose::OFF,
            timeout: None,
            nodelay,
            user_agent,
        }
    }

    #[cfg(feature = "__rustls")]
    pub(crate) fn new_rustls_tls<T>(
        mut http: HttpConnector,
        tls: rustls::ClientConfig,
        proxies: Arc<Vec<Proxy>>,
        user_agent: Option<HeaderValue>,
        local_addr: T,
        nodelay: bool,
    ) -> Connector
    where
        T: Into<Option<IpAddr>>,
    {
        http.set_local_address(local_addr.into());
        http.enforce_http(false);

        let (tls, tls_proxy) = if proxies.is_empty() {
            let tls = Arc::new(tls);
            (tls.clone(), tls)
        } else {
            let mut tls_proxy = tls.clone();
            tls_proxy.alpn_protocols.clear();
            (Arc::new(tls), Arc::new(tls_proxy))
        };

        Connector {
            inner: Inner::RustlsTls {
                http,
                tls,
                tls_proxy,
            },
            proxies,
            verbose: verbose::OFF,
            timeout: None,
            nodelay,
            user_agent,
        }
    }

    pub(crate) fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    // pub(crate) fn set_verbose(&mut self, enabled: bool) {
    //     self.verbose.0 = enabled;
    // }

    // #[cfg(feature = "socks")]
    // fn connect_socks(&self, dst: Uri, proxy: ProxyScheme) -> Result<Conn, BoxError> {
    //     let dns = match proxy {
    //         ProxyScheme::Socks5 {
    //             remote_dns: false, ..
    //         } => socks::DnsResolve::Local,
    //         ProxyScheme::Socks5 {
    //             remote_dns: true, ..
    //         } => socks::DnsResolve::Proxy,
    //         ProxyScheme::Http { .. } | ProxyScheme::Https { .. } => {
    //             unreachable!("connect_socks is only called for socks proxies");
    //         }
    //     };

    //     match &self.inner {
    //         #[cfg(feature = "default-tls")]
    //         Inner::DefaultTls(_http, tls) => {
    //             if dst.scheme() == Some(&Scheme::HTTPS) {
    //                 let host = dst.host().ok_or("no host in url")?.to_string();
    //                 let conn = socks::connect(proxy, dst, dns);
    //                 let tls_connector = tokio_native_tls::TlsConnector::from(tls.clone());
    //                 let io = tls_connector.connect(&host, conn);
    //                 return Ok(Conn {
    //                     inner: self.verbose.wrap(NativeTlsConn { inner: io }),
    //                     is_proxy: false,
    //                 });
    //             }
    //         }
    //         #[cfg(feature = "__rustls")]
    //         Inner::RustlsTls { tls_proxy, .. } => {
    //             if dst.scheme() == Some(&Scheme::HTTPS) {
    //                 use std::convert::TryFrom;

    //                 let tls = tls_proxy.clone();
    //                 let host = dst.host().ok_or("no host in url")?.to_string();
    //                 let conn = socks::connect(proxy, dst, dns);
    //                 let server_name = rustls::ServerName::try_from(host.as_str())
    //                     .map_err(|_| "Invalid Server Name")?;
    //                 let io = RustlsConnector::from(tls)
    //                     .connect(server_name, conn)
    //                     ;
    //                 return Ok(Conn {
    //                     inner: self.verbose.wrap(RustlsTlsConn { inner: io }),
    //                     is_proxy: false,
    //                 });
    //             }
    //         }
    //         #[cfg(not(feature = "__tls"))]
    //         Inner::Http(_) => (),
    //     }

    //     socks::connect(proxy, dst, dns).map(|tcp| Conn {
    //         inner: self.verbose.wrap(tcp),
    //         is_proxy: false,
    //     })
    // }

    // fn connect_with_maybe_proxy(self, dst: Uri, is_proxy: bool) -> Result<Conn, BoxError> {
    //     match self.inner {
    //         #[cfg(not(feature = "__tls"))]
    //         Inner::Http(mut http) => {
    //             let io = http.call(dst);
    //             Ok(Conn {
    //                 inner: self.verbose.wrap(io),
    //                 is_proxy,
    //             })
    //         }
    //         #[cfg(feature = "default-tls")]
    //         Inner::DefaultTls(http, tls) => {
    //             let mut http = http.clone();

    //             // Disable Nagle's algorithm for TLS handshake
    //             //
    //             // https://www.openssl.org/docs/man1.1.1/man3/SSL_connect.html#NOTES
    //             if !self.nodelay && (dst.scheme() == Some(&Scheme::HTTPS)) {
    //                 http.set_nodelay(true);
    //             }

    //             let tls_connector = tokio_native_tls::TlsConnector::from(tls.clone());
    //             let mut http = hyper_tls::HttpsConnector::from((http, tls_connector));
    //             let io = http.call(dst);

    //             if let hyper_tls::MaybeHttpsStream::Https(stream) = io {
    //                 if !self.nodelay {
    //                     stream.get_ref().get_ref().get_ref().set_nodelay(false)?;
    //                 }
    //                 Ok(Conn {
    //                     inner: self.verbose.wrap(NativeTlsConn { inner: stream }),
    //                     is_proxy,
    //                 })
    //             } else {
    //                 Ok(Conn {
    //                     inner: self.verbose.wrap(io),
    //                     is_proxy,
    //                 })
    //             }
    //         }
    //         #[cfg(feature = "__rustls")]
    //         Inner::RustlsTls { http, tls, .. } => {
    //             let mut http = http.clone();

    //             // Disable Nagle's algorithm for TLS handshake
    //             //
    //             // https://www.openssl.org/docs/man1.1.1/man3/SSL_connect.html#NOTES
    //             if !self.nodelay && (dst.scheme() == Some(&Scheme::HTTPS)) {
    //                 http.set_nodelay(true);
    //             }

    //             let mut http = hyper_rustls::HttpsConnector::from((http, tls.clone()));
    //             let io = http.call(dst);

    //             if let hyper_rustls::MaybeHttpsStream::Https(stream) = io {
    //                 if !self.nodelay {
    //                     let (io, _) = stream.get_ref();
    //                     io.set_nodelay(false)?;
    //                 }
    //                 Ok(Conn {
    //                     inner: self.verbose.wrap(RustlsTlsConn { inner: stream }),
    //                     is_proxy,
    //                 })
    //             } else {
    //                 Ok(Conn {
    //                     inner: self.verbose.wrap(io),
    //                     is_proxy,
    //                 })
    //             }
    //         }
    //     }
    // }

    // fn connect_via_proxy(self, dst: Uri, proxy_scheme: ProxyScheme) -> Result<Conn, BoxError> {
    //     lunatic_log::debug!("proxy({:?}) intercepts '{:?}'", proxy_scheme, dst);

    //     let (proxy_dst, _auth) = match proxy_scheme {
    //         ProxyScheme::Http { host, auth } => (into_uri(Scheme::HTTP, host), auth),
    //         ProxyScheme::Https { host, auth } => (into_uri(Scheme::HTTPS, host), auth),
    //         #[cfg(feature = "socks")]
    //         ProxyScheme::Socks5 { .. } => return self.connect_socks(dst, proxy_scheme),
    //     };

    //     #[cfg(feature = "__tls")]
    //     let auth = _auth;

    //     match &self.inner {
    //         #[cfg(feature = "default-tls")]
    //         Inner::DefaultTls(http, tls) => {
    //             if dst.scheme() == Some(&Scheme::HTTPS) {
    //                 let host = dst.host().to_owned();
    //                 let port = dst.port().map(|p| p.as_u16()).unwrap_or(443);
    //                 let http = http.clone();
    //                 let tls_connector = tokio_native_tls::TlsConnector::from(tls.clone());
    //                 let mut http = hyper_tls::HttpsConnector::from((http, tls_connector));
    //                 let conn = http.call(proxy_dst);
    //                 lunatic_log::trace!("tunneling HTTPS over proxy");
    //                 let tunneled = tunnel(
    //                     conn,
    //                     host.ok_or("no host in url")?.to_string(),
    //                     port,
    //                     self.user_agent.clone(),
    //                     auth,
    //                 );
    //                 let tls_connector = tokio_native_tls::TlsConnector::from(tls.clone());
    //                 let io = tls_connector.connect(host.ok_or("no host in url")?, tunneled);
    //                 return Ok(Conn {
    //                     inner: self.verbose.wrap(NativeTlsConn { inner: io }),
    //                     is_proxy: false,
    //                 });
    //             }
    //         }
    //         #[cfg(feature = "__rustls")]
    //         Inner::RustlsTls {
    //             http,
    //             tls,
    //             tls_proxy,
    //         } => {
    //             if dst.scheme() == Some(&Scheme::HTTPS) {
    //                 use rustls::ServerName;
    //                 use std::convert::TryFrom;
    //                 use tokio_rustls::TlsConnector as RustlsConnector;

    //                 let host = dst.host().ok_or("no host in url")?.to_string();
    //                 let port = dst.port().map(|r| r.as_u16()).unwrap_or(443);
    //                 let http = http.clone();
    //                 let mut http = hyper_rustls::HttpsConnector::from((http, tls_proxy.clone()));
    //                 let tls = tls.clone();
    //                 let conn = http.call(proxy_dst);
    //                 lunatic_log::trace!("tunneling HTTPS over proxy");
    //                 let maybe_server_name =
    //                     ServerName::try_from(host.as_str()).map_err(|_| "Invalid Server Name");
    //                 let tunneled = tunnel(conn, host, port, self.user_agent.clone(), auth);
    //                 let server_name = maybe_server_name?;
    //                 let io = RustlsConnector::from(tls).connect(server_name, tunneled);

    //                 return Ok(Conn {
    //                     inner: self.verbose.wrap(RustlsTlsConn { inner: io }),
    //                     is_proxy: false,
    //                 });
    //             }
    //         }
    //         #[cfg(not(feature = "__tls"))]
    //         Inner::Http(_) => (),
    //     }

    //     self.connect_with_maybe_proxy(proxy_dst, true)
    // }

    pub fn set_keepalive(&mut self, dur: Option<Duration>) {
        self.keep_alive = dur;
        // match &mut self.inner {
        //     #[cfg(feature = "default-tls")]
        //     Inner::DefaultTls(http, _tls) => http.set_keepalive(dur),
        //     #[cfg(feature = "__rustls")]
        //     Inner::RustlsTls { http, .. } => http.set_keepalive(dur),
        //     #[cfg(not(feature = "__tls"))]
        //     Inner::Http(http) => http.set_keepalive(dur),
        // }
    }
}

fn into_uri(scheme: Scheme, host: Authority) -> Uri {
    // TODO: Should the `http` crate get `From<(Scheme, Authority)> for Uri`?
    http::Uri::builder()
        .scheme(scheme)
        .authority(host)
        .path_and_query(http::uri::PathAndQuery::from_static("/"))
        .build()
        .expect("scheme and authority is valid Uri")
}

// fn with_timeout<T, F>(f: F, timeout: Option<Duration>) -> Result<T, BoxError> {
//     if let Some(to) = timeout {
//         match tokio::time::timeout(to, f) {
//             Err(_elapsed) => Err(Box::new(crate::error::TimedOut) as BoxError),
//             Ok(Ok(try_res)) => Ok(try_res),
//             Ok(Err(e)) => Err(e),
//         }
//     } else {
//         f
//     }
// }

// impl Service<Uri> for Connector {
//     type Response = Conn;
//     type Error = BoxError;
//     type Future = Connecting;

//     fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
//         Poll::Ready(Ok(()))
//     }

//     fn call(&mut self, dst: Uri) -> Self::Future {
//         lunatic_log::debug!("starting new connection: {:?}", dst);
//         let timeout = self.timeout;
//         for prox in self.proxies.iter() {
//             if let Some(proxy_scheme) = prox.intercept(&dst) {
//                 return Box::pin(with_timeout(
//                     self.clone().connect_via_proxy(dst, proxy_scheme),
//                     timeout,
//                 ));
//             }
//         }

//         Box::pin(with_timeout(
//             self.clone().connect_with_maybe_proxy(dst, false),
//             timeout,
//         ))
//     }
// }

/// Note: the `is_proxy` member means *is plain text HTTP proxy*.
/// This tells hyper whether the URI should be written in
/// * origin-form (`GET /just/a/path HTTP/1.1`), when `is_proxy == false`, or
/// * absolute-form (`GET http://foo.bar/and/a/path HTTP/1.1`), otherwise.
pub(crate) struct Conn {
    inner: TcpStream,
    is_proxy: bool,
}

#[derive(Clone)]
pub(crate) struct DnsResolverWithOverrides {
    // dns_resolver: Resolver,
    overrides: Arc<HashMap<String, Vec<SocketAddr>>>,
}

impl DnsResolverWithOverrides {
    fn new(overrides: HashMap<String, Vec<SocketAddr>>) -> Self {
        DnsResolverWithOverrides {
            // dns_resolver,
            overrides: Arc::new(overrides),
        }
    }
}

mod verbose {
    use std::fmt;

    struct Verbose<T> {
        id: u32,
        inner: T,
    }

    struct Escape<'a>(&'a [u8]);

    impl fmt::Debug for Escape<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "b\"")?;
            for &c in self.0 {
                // https://doc.rust-lang.org/reference.html#byte-escapes
                if c == b'\n' {
                    write!(f, "\\n")?;
                } else if c == b'\r' {
                    write!(f, "\\r")?;
                } else if c == b'\t' {
                    write!(f, "\\t")?;
                } else if c == b'\\' || c == b'"' {
                    write!(f, "\\{}", c as char)?;
                } else if c == b'\0' {
                    write!(f, "\\0")?;
                // ASCII printable
                } else if c >= 0x20 && c < 0x7f {
                    write!(f, "{}", c as char)?;
                } else {
                    write!(f, "\\x{:02x}", c)?;
                }
            }
            write!(f, "\"")?;
            Ok(())
        }
    }
}

#[cfg(feature = "__tls")]
#[cfg(test)]
mod tests {
    use super::tunnel;
    use crate::proxy;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use tokio::net::TcpStream;
    use tokio::runtime;

    static TUNNEL_UA: &str = "tunnel-test/x.y";
    static TUNNEL_OK: &[u8] = b"\
        HTTP/1.1 200 OK\r\n\
        \r\n\
    ";

    macro_rules! mock_tunnel {
        () => {{
            mock_tunnel!(TUNNEL_OK)
        }};
        ($write:expr) => {{
            mock_tunnel!($write, "")
        }};
        ($write:expr, $auth:expr) => {{
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let connect_expected = format!(
                "\
                 CONNECT {0}:{1} HTTP/1.1\r\n\
                 Host: {0}:{1}\r\n\
                 User-Agent: {2}\r\n\
                 {3}\
                 \r\n\
                 ",
                addr.ip(),
                addr.port(),
                TUNNEL_UA,
                $auth
            )
            .into_bytes();

            thread::spawn(move || {
                let (mut sock, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let n = sock.read(&mut buf).unwrap();
                assert_eq!(&buf[..n], &connect_expected[..]);

                sock.write_all($write).unwrap();
            });
            addr
        }};
    }

    fn ua() -> Option<http::header::HeaderValue> {
        Some(http::header::HeaderValue::from_static(TUNNEL_UA))
    }

    #[test]
    fn test_tunnel() {
        let addr = mock_tunnel!();

        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("new rt");
        let f = async move {
            let tcp = TcpStream::connect(&addr);
            let host = addr.ip().to_string();
            let port = addr.port();
            tunnel(tcp, host, port, ua(), None)
        };

        rt.block_on(f).unwrap();
    }

    #[test]
    fn test_tunnel_eof() {
        let addr = mock_tunnel!(b"HTTP/1.1 200 OK");

        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("new rt");
        let f = async move {
            let tcp = TcpStream::connect(&addr);
            let host = addr.ip().to_string();
            let port = addr.port();
            tunnel(tcp, host, port, ua(), None)
        };

        rt.block_on(f).unwrap_err();
    }

    #[test]
    fn test_tunnel_non_http_response() {
        let addr = mock_tunnel!(b"foo bar baz hallo");

        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("new rt");
        let f = async move {
            let tcp = TcpStream::connect(&addr);
            let host = addr.ip().to_string();
            let port = addr.port();
            tunnel(tcp, host, port, ua(), None)
        };

        rt.block_on(f).unwrap_err();
    }

    #[test]
    fn test_tunnel_proxy_unauthorized() {
        let addr = mock_tunnel!(
            b"\
            HTTP/1.1 407 Proxy Authentication Required\r\n\
            Proxy-Authenticate: Basic realm=\"nope\"\r\n\
            \r\n\
        "
        );

        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("new rt");
        let f = async move {
            let tcp = TcpStream::connect(&addr);
            let host = addr.ip().to_string();
            let port = addr.port();
            tunnel(tcp, host, port, ua(), None)
        };

        let error = rt.block_on(f).unwrap_err();
        assert_eq!(error.to_string(), "proxy authentication required");
    }

    #[test]
    fn test_tunnel_basic_auth() {
        let addr = mock_tunnel!(
            TUNNEL_OK,
            "Proxy-Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==\r\n"
        );

        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("new rt");
        let f = async move {
            let tcp = TcpStream::connect(&addr);
            let host = addr.ip().to_string();
            let port = addr.port();
            tunnel(
                tcp,
                host,
                port,
                ua(),
                Some(proxy::encode_basic_auth("Aladdin", "open sesame")),
            )
        };

        rt.block_on(f).unwrap();
    }
}
