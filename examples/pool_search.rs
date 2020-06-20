use futures::prelude::*;
use rs621::{client::Client, pool::PoolSearch};

#[tokio::main]
async fn main() -> rs621::error::Result<()> {
    let client = Client::new("https://e621.net", "MyProject/1.0 (by username on e621)")?;

    println!("Pools by Lynxgriffin!");

    let mut pool_stream = client.pool_search(PoolSearch::new().name_matches("Lynxgriffin"));

    while let Some(pool) = pool_stream.next().await {
        match pool {
            Ok(pool) => println!("- {}", pool.name),
            Err(e) => println!("- couldn't load pool: {}", e),
        }
    }

    Ok(())
}
