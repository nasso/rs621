# rs621

[![build](https://github.com/nasso/rs621/actions/workflows/rust.yml/badge.svg)](https://github.com/nasso/rs621/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/rs621.svg)](https://crates.io/crates/rs621)
[![Docs.rs](https://docs.rs/rs621/badge.svg)](https://docs.rs/rs621)

Rust bindings for the [e621.net](https://e926.net) API.

E621 is a large online archive of furry (anthropomorphic) art. `rs621` provides
easy-to-use bindings to its public HTTP API. It uses the `reqwest` crate to make
HTTPs requests and exposes an asynchronous API.

## Features

- Highly asynchronous
- Convenient stream-based API.
- Post listing and searching, using any of the search options from the website.
- Pool listing and searching.
- Unlimited result count (automatically makes more requests in sequence to go
  beyond the API limit of 320 posts per request).
- Automatic rate-limit throttling.
- Bulk-oriented API.

## Usage

Note: the API is highly asynchronous. If you're not familiar with those
concepts, check out
[Asynchronous Programming in Rust](https://rust-lang.github.io/async-book/).

First, create a [`Client`]. You'll need to provide the domain URL you'd like to
use, without the final slash (most likely [https://e926.net](https://e926.net)
or its unsafe counterpart). You also have to provide a descriptive User-Agent
for your project. The official API encourages you to include your E621 username
so that you may be contacted if your project causes problems.

```rust
let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
```

You can now use that client to make various operations, like a basic search,
with [`Client::post_search`]. The function returns a [`Stream`], which is like
an asynchronous version of [`Iterator`].

```rust
use futures::prelude::*;

let mut post_stream = client.post_search(&["fluffy", "order:score"][..]).take(20);

while let Some(post) = post_stream.next().await {
    println!("Post #{}", post?.id);
}
```

If you have a list of post IDs:

```rust
let mut post_stream = client.get_posts(&[8595, 535, 2105, 1470]);

while let Some(post) = post_stream.next().await {
    println!("Post #{}", post?.id);
}
```

Best effort should be made to make as few API requests as possible. `rs621`
helps by providing bulk-oriented methods that take care of this for you. For
example, if you have 400 post IDs you'd like to fetch, a single call to
[`Client::get_posts`] should be enough and WILL be faster. Do NOT call it
repeatedly in a loop.

[`client`]: client/struct.Client.html
[`client::post_search`]: client/struct.Client.html#method.post_search
[`stream`]: https://docs.rs/futures/0.3.5/futures/stream/trait.Stream.html
[`iterator`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html
[`client::get_posts`]: client/struct.Client.html#method.get_posts

## Requirements

`rs621` uses the `rust-openssl` crate. It has some requirements:

On Linux:

- OpenSSL 1.0.1, 1.0.2, or 1.1.0 with headers (see
  [rust-openssl](https://github.com/sfackler/rust-openssl)).

On Windows and macOS:

- Nothing.

See [reqwest on crates.io](https://crates.io/crates/reqwest) for more details.

## License

`rs621` is licensed under the terms of both the MIT license and the Apache
License (Version 2.0), at your choice.

See LICENSE-MIT and LICENSE-APACHE-2.0 files for the full texts.
