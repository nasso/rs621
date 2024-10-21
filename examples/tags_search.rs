use futures::prelude::*;
use rs621::{
    client::Client,
    tag::{Order, Query},
};

#[tokio::main]
async fn main() -> rs621::error::Result<()> {
    let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;

    println!("Top ten tags by post count!");

    let result_stream = client
        .tag_search(Query::new().per_page(1).order(Order::Count))
        .take(10);

    futures::pin_mut!(result_stream);

    while let Some(tag) = result_stream.next().await {
        match tag {
            Ok(tag) => println!("- {} with a score of {}", tag.name, tag.post_count),
            Err(e) => println!("- couldn't load tag: {}", e),
        }
    }

    Ok(())
}
