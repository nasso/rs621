//! Wrapper crate for the [e621.net](https://e926.net) API.
//!
//! ## Usage
//!
//! Note: the API is highly asynchronous. If you're not familiar with those concepts, check out
//! [Asynchronous Programming in Rust](https://rust-lang.github.io/async-book/).
//!
//! First, create a [`Client`]. You'll need to provide the domain URL you'd like to use, without the
//! final slash (most likely [https://e926.net](https://e926.net) or its unsafe counterpart).  You
//! also have to provide a descriptive User-Agent for your project. The official API encourages you
//! to include your E621 username so that you may be contacted if your project causes problems.
//!
//! ```no_run
//! # use rs621::client::Client;
//! # fn main() -> Result<(), rs621::error::Error> {
//! let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
//! # Ok(()) }
//! ```
//!
//! You can now use that client to make various operations, like a basic search, with
//! [`Client::post_search`]. The function returns a [`Stream`], which is like an asynchronous
//! version of [`Iterator`].
//!
//! ```no_run
//! # use rs621::client::Client;
//! use futures::prelude::*;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), rs621::error::Error> {
//! # let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
//! let mut post_stream = client.post_search(&["fluffy", "order:score"][..]).take(20);
//!
//! while let Some(post) = post_stream.next().await {
//!     println!("Post #{}", post?.id);
//! }
//! # Ok(()) }
//! ```
//!
//! If you have a list of post IDs:
//!
//! ```no_run
//! # use rs621::client::Client;
//! # use futures::prelude::*;
//! # #[tokio::main]
//! # async fn main() -> Result<(), rs621::error::Error> {
//! # let client = Client::new("https://e926.net", "MyProject/1.0 (by username on e621)")?;
//! let mut post_stream = client.get_posts(&[8595, 535, 2105, 1470]);
//!
//! while let Some(post) = post_stream.next().await {
//!     println!("Post #{}", post?.id);
//! }
//! # Ok(()) }
//! ```
//!
//! Best effort should be made to make as few API requests as possible. `rs621` helps by providing
//! bulk-oriented methods that take care of this for you. For example, if you have 400 post IDs
//! you'd like to fetch, a single call to [`Client::get_posts`] should be enough and WILL be
//! faster. Do NOT call it repeatedly in a loop.
//!
//! ## Notes from the official API:
//!
//! ### User Agents
//!
//! > A non-empty User-Agent header is required for all requests. Please pick a descriptive
//! > User-Agent for your project. You are encouraged to include your e621 username so that you may
//! > be contacted if your project causes problems. **DO NOT impersonate a browser user agent, as
//! > this will get you blocked.** An example user-agent would be
//! > ```text
//! > MyProject/1.0 (by username on e621)
//! > ```
//! > Due to frequent abuse, default user agents for programming languages and libraries are usually
//! > blocked. Please make sure that you are defining a user agent that is in line with our policy.
//! >
//! > [[...]](https://e926.net/help/show/api#basics)
//!
//! Thus, `rs621` doesn't have a default user agent and you are required to specify your own.
//!
//! ### Rate Limiting
//!
//! > E621/E926 have a hard rate limit of two requests per second. This is a hard upper limit and if
//! > you are hitting it, you are already going way too fast. Hitting the rate limit will result in
//! > a 503 HTTP response code. You should make a best effort not to make more than one request per
//! > second over a sustained period.
//!
//! `rs621` will enforce this limit with a short sleeping time after every API request being made.
//!
//! [`Client`]: client/struct.Client.html
//! [`Client::post_search`]: client/struct.Client.html#method.post_search
//! [`Stream`]: https://docs.rs/futures/0.3.5/futures/stream/trait.Stream.html
//! [`Iterator`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html
//! [`Client::get_posts`]: client/struct.Client.html#method.get_posts

/// Client related structures.
pub mod client;

/// Error management.
pub mod error;

/// Post management.
pub mod post;

/// Pool management.
pub mod pool;

/// Tag management.
pub mod tag;
