# rs621
[![Build Status](https://travis-ci.com/nasso/rs621.svg?branch=master)](
https://travis-ci.com/nasso/rs621)
[![codecov](https://codecov.io/gh/nasso/rs621/branch/master/graph/badge.svg)](
https://codecov.io/gh/nasso/rs621)
[![Crates.io](https://img.shields.io/crates/v/rs621.svg)](
https://crates.io/crates/rs621)
[![Docs.rs](https://docs.rs/rs621/badge.svg)](https://docs.rs/rs621)
[![Telegram](https://img.shields.io/badge/Telegram-Join%20Chat-blue.svg)](
https://t.me/rs621)

Rust bindings for the [e621.net](https://e926.net) API.

E621 is a large online archive of furry (anthropomorphic) art. `rs621` provides
easy-to-use bindings to its public HTTP API. It uses the `reqwest` crate to make
the requests over HTTPS.

## Features

- Convenient iterator based API.
- Post listing and searching, using any of the search options from the website.
- Pool listing and searching.
- Unlimited result count (automatically makes more requests in sequence to go
    beyond the API limit of 320 posts per request).

## Usage

First, create a `Client`. You have to provide a descriptive User-Agent for your
project. The official API encourages you to include your E621 username so that
you may be contacted if your project causes problems.

```rust
let client = Client::new("MyProject/1.0 (by username on e621)")?;
```

Now it's ready to go! For example you can get post #8595 like this:

```rust
let post = client.get_post(8595)?;

assert_eq!(post.id, 8595);
```

Or you can make a search like on the website, using tags:

```rust
println!("A list of cool fluffy posts:");
for post in client.post_search(&["fluffy", "rating:s"][..]).take(20) {
    println!("#{}", post?.id);
}
```

## Requirements

`rs621` uses the `rust-openssl` crate. It has some
requirements:

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
