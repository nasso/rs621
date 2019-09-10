use rs621::client::Client;

fn main() -> rs621::error::Result<()> {
    let client = Client::new("MyProject/1.0 (by username on e621)")?;

    println!("Top ten safe fluffy posts!");

    for post in client
        .post_search(&["fluffy", "rating:s", "order:score"][..])
        .take(10)
    {
        match post {
            Ok(post) => println!("- #{} with a score of {}", post.id, post.score),
            Err(e) => println!("- couldn't load post: {}", e),
        }
    }

    Ok(())
}
