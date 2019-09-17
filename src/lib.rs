//! Wrapper crate for the [e621.net](https://e926.net) API.
//!
//! ## Usage
//!
//! First, create a [`Client`]. You have to provide a descriptive User-Agent for your project. The
//! official API encourages you to include your E621 username so that you may be contacted if your
//! project causes problems.
//!
//! ```no_run
//! # use rs621::client::Client;
//! # fn main() -> Result<(), rs621::error::Error> {
//! let client = Client::new("MyProject/1.0 (by username on e621)")?;
//! # Ok(()) }
//! ```
//!
//! Now it's ready to go! For example you can get post #8595 like this:
//!
//! ```no_run
//! # use rs621::client::Client;
//! # fn main() -> Result<(), rs621::error::Error> {
//! # let client = Client::new("MyProject/1.0 (by username on e621)")?;
//! let post = client.get_post(8595)?;
//!
//! assert_eq!(post.id, 8595);
//! # Ok(()) }
//! ```
//!
//! Or you can make a search like on the website, using tags:
//!
//! ```no_run
//! # use rs621::client::Client;
//! # fn main() -> Result<(), rs621::error::Error> {
//! # let client = Client::new("MyProject/1.0 (by username on e621)")?;
//! for post in client.post_search(&["fluffy", "rating:s"][..]).take(20) {
//!     println!("#{}", post?.id);
//! }
//! # Ok(()) }
//! ```
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
//! This short sleep time only happens in the thread the request is being made and requests made in
//! other threads will NOT be affected. Thus, **if you are using `rs621` across multiple threads,
//! you are responsible for making sure that you aren't exceeding the upper rate limit.** Waiting
//! for functions performing requests in other threads to return should be enough.
//!
//! [`Client`]: client/struct.Client.html

mod utils;

/// Client related structures.
pub mod client;

/// Error management.
pub mod error;

/// Post management.
pub mod post;

/// Pool management.
pub mod pool;
