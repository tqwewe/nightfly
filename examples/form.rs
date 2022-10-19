use lunatic::Mailbox;

// Short example of a POST request with form data.
//
//
#[lunatic::main]
fn main(_: Mailbox<()>) {
    let response = nightfly::Client::new()
        .post("http://www.baidu.com")
        .form(&[("one", "1")])
        .send()
        .expect("send");
    println!("Response status {}", response.status());
}
