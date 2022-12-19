use futures::Future;

#[derive(Debug, Clone, Default)]
pub struct RateLimit {}

impl RateLimit {
    pub async fn check<F, R>(self, fut: F) -> R
    where
        F: Future<Output = R>,
    {
        fut.await
    }
}
