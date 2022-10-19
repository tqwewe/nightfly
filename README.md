# nightfly

## Disclaimer
This project is still highly experimental and therefore not to be used in production

[![crates.io](https://img.shields.io/crates/v/nightfly.svg)](https://crates.io/crates/nightfly)
[![Documentation](https://docs.rs/nightfly/badge.svg)](https://docs.rs/nightfly)
[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/nightfly.svg)](./LICENSE-APACHE)
[![CI](https://github.com/seanmonstar/nightfly/workflows/CI/badge.svg)](https://github.com/seanmonstar/nightfly/actions?query=workflow%3ACI)

An ergonomic, batteries-included HTTP Client for Rust.

- Plain bodies, JSON, urlencoded, multipart
- Customizable redirect policy
- HTTP Proxies
- HTTPS via lunatic-native TLS
- Cookie Store
- [Changelog](CHANGELOG.md)


## Example

This example uses [Lunatic](https://lunatic.rs) and enables some
optional features, so your `Cargo.toml` could look like this:

```toml
[dependencies]
nightfly = { version = "0.1.0" }
lunatic = { version = "0.11" }
```

And then the code:

```rust,no_run
use std::collections::HashMap;

#[lunatic::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resp = nightfly::get("https://httpbin.org/ip")
        
        .json::<HashMap<String, String>>()
        ;
    println!("{:#?}", resp);
    Ok(())
}
```

## Blocking Client

There is an optional "blocking" client API that can be enabled:

```toml
[dependencies]
nightfly = { version = "0.11", features = ["blocking", "json"] }
```

```rust,no_run
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resp = nightfly::blocking::get("https://httpbin.org/ip")?
        .json::<HashMap<String, String>>()?;
    println!("{:#?}", resp);
    Ok(())
}
```

## Requirements

On Linux:

- OpenSSL 1.0.1, 1.0.2, 1.1.0, or 1.1.1 with headers (see https://github.com/sfackler/rust-openssl)

On Windows and macOS:

- Nothing.

Reqwest uses [rust-native-tls](https://github.com/sfackler/rust-native-tls),
which will use the operating system TLS framework if available, meaning Windows
and macOS. On Linux, it will use OpenSSL 1.1.


## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
