use rs621::client::Client;

fn main() -> rs621::error::Result<()> {
    let client = Client::new("MyProject/1.0 (by username on e621)")?;

    println!("Top ten safe fluffy posts!");

    for post in client
        .list(&["fluffy", "rating:s", "order:score"][..])
        .take(10)
    {
        let post = post?;
        println!("- #{} with a score of {}", post.id, post.score);
    }

    Ok(())
}
