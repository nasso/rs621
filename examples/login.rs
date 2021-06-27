use futures::prelude::*;
use rs621::client::Client;

macro_rules! input {
    ($($arg:tt)*) => ({
        use std::io::prelude::*;

        print!($($arg)*);

        let mut buffer = String::new();

        std::io::stdout()
            .flush()
            .and_then(|_| std::io::stdin().read_line(&mut buffer))
            .map(move |_| if buffer.trim().is_empty() {
                None
            } else {
                Some(String::from(buffer.trim()))
            })
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = "https://e926.net";
    let server = input!("Server ({}): ", server)?.unwrap_or_else(|| server.into());

    let login = input!("Login (optional): ")?;
    let api_key = login.as_ref().map(|_| input!("API key: "));

    let mut client = Client::new(&server, "MyProject/1.0 (by username on e621)")?;

    if let (Some(login), Some(Ok(Some(api_key)))) = (login, api_key) {
        client.login(login, api_key);
    }

    let tags = input!("Search terms: ")?.unwrap_or_else(|| "".into());

    let mut result_stream = client
        .post_search(&tags.split_ascii_whitespace().collect::<Vec<_>>()[..])
        .take(10);

    while let Some(post) = result_stream.next().await {
        match post {
            Ok(post) => println!("- #{}: {:?}", post.id, post.file.url),
            Err(e) => println!("- couldn't load post: {}", e),
        }
    }

    Ok(())
}
