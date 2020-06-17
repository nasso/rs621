use futures::prelude::*;
use rs621::client::Client;

#[tokio::main]
async fn main() -> rs621::error::Result<()> {
    let client = Client::new("https://e621.net", "MyProject/1.0 (by username on e621)")?;

    println!("Top ten safe fluffy posts!");

    let mut result_stream = client
        .post_search(&["fluffy", "rating:s", "order:score"][..])
        .take(10);

    while let Some(post) = result_stream.next().await {
        match post {
            Ok(post) => println!("- #{} with a score of {}", post.id, post.score),
            Err(e) => println!("- couldn't load post: {}", e),
        }
    }

    Ok(())
}
