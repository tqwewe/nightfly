#![deny(warnings)]

// This is using the `lunatic` runtime.
//
#[lunatic::main]
fn main() -> Result<(), reqwest::Error> {
    // Make sure you are running tor and this is your socks port
    let proxy = reqwest::Proxy::all("socks5h://127.0.0.1:9050").expect("tor proxy should be there");
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .build()
        .expect("should be able to build reqwest client");

    let res = client.get("https://check.torproject.org").send();
    println!("Status: {}", res.status());

    let text = res.text();
    let is_tor = text.contains("Congratulations. This browser is configured to use Tor.");
    println!("Is Tor: {}", is_tor);
    assert!(is_tor);

    Ok(())
}
