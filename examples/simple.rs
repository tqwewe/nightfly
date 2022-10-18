#![deny(warnings)]
extern crate reqwest;

use lunatic::Mailbox;

// This is using the `lunatic` runtime.
//
#[lunatic::main]
fn main(_: Mailbox<()>) -> () {
    // Some simple CLI args requirements...
    let url = match std::env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("No CLI URL provided, using default.");
            // "https://hyper.rs".into()
            "http://localhost:3000".into()
        }
    };

    eprintln!("Fetching {:?}...", url);

    // reqwest::get() is a convenience function.
    //
    // In most cases, you should create/build a reqwest::Client and reuse
    // it for all requests.
    let res = reqwest::get(url).unwrap();

    eprintln!("Response: {:?} {}", res.version(), res.status());
    eprintln!("Headers: {:#?}\n", res.headers());

    let body = res.text().unwrap();

    println!("{}", body);

    // Ok(())
}
