# rs621
[![Crates.io](https://img.shields.io/crates/v/rs621.svg)](
https://crates.io/crates/rs621)
[![Docs.rs](https://docs.rs/rs621/badge.svg)](https://docs.rs/rs621)
[![Build Status](https://travis-ci.com/nasso/rs621.svg?branch=master)](
https://travis-ci.com/nasso/rs621)
[![codecov](https://codecov.io/gh/nasso/rs621/branch/master/graph/badge.svg)](
https://codecov.io/gh/nasso/rs621)
[![GitHub license](
https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/nasso/rs621/blob/master/README.md#license)

Rust bindings for the [e621.net](https://e926.net) API.

E621 is a large online archive of furry (anthropomorphic) art. `rs621` provides
easy-to-use bindings to its public HTTP API. It uses the `reqwest` crate to make
the requests over HTTPS.

## Features

- Regular tag searching, using any of the search options from the website.
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
for post in client.list(&["fluffy", "rating:s"][..], 5).take(20) {
    println!("{}", post);
}
```

## Requirements

`rs621` uses the reqwest crate, which itself uses rust-openssl. It has some
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
