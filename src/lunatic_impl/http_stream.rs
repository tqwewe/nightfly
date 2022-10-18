use std::io::{Read, Write};

use lunatic::net::{TcpStream, ToSocketAddrs};
use serde::{Deserialize, Serialize};

use crate::error::Kind;

#[derive(Clone, Serialize, Deserialize)]
pub enum HttpStream {
    Tcp(TcpStream),
    // Tls(TlsStream),
}

impl HttpStream {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> crate::Result<HttpStream> {
        // TODO: handle tls
        match TcpStream::connect(addr) {
            Ok(stream) => Ok(HttpStream::Tcp(stream)),
            Err(e) => {
                eprintln!("ERROR IN CONNECT {:?}", e);
                Err(crate::Error::new(
                    Kind::Builder,
                    Some("Failed to connect".to_string()),
                ))
            }
        }
    }
}

impl Read for HttpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            HttpStream::Tcp(stream) => stream.read(buf),
        }
    }
}

impl Write for HttpStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            HttpStream::Tcp(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            HttpStream::Tcp(stream) => stream.flush(),
        }
    }
}
