use futures::prelude::*;
use rs621::client::Client;

#[tokio::main]
async fn main() -> rs621::error::Result<()> {
    let client = Client::new("https://e621.net", "MyProject/1.0 (by username on e621)")?;

    println!("Some very specific posts fetched by ID:");

    let mut post_stream = client.get_posts(&[8595, 535, 2105, 1470]);

    while let Some(post) = post_stream.next().await {
        match post {
            Ok(post) => println!("- #{} with a score of {}", post.id, post.score.total),
            Err(e) => println!("- couldn't load post: {}", e),
        }
    }

    Ok(())
}
