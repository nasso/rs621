use futures::Future;

use std::sync::Arc;

use tokio::sync::{Mutex, MutexGuard};
use tokio::time::{sleep_until, Duration, Instant};

/// Forced cool down duration performed at every request. E621 allows at most 2 requests per second,
/// so the lowest safe value we can have here is 500 ms.
const REQ_COOLDOWN_DURATION: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, Default)]
pub struct RateLimit {
    // Use a tokio mutex for fairness and because ~500ms is crazy long to block
    // an async task.
    deadline: Arc<Mutex<Option<Instant>>>,
}

struct Guard<'a>(MutexGuard<'a, Option<Instant>>);

impl<'a> Drop for Guard<'a> {
    fn drop(&mut self) {
        // Use a `Drop` impl so that updating the deadline is panic-safe.
        *self.0 = Some(Instant::now() + REQ_COOLDOWN_DURATION);
    }
}

impl RateLimit {
    async fn lock(&self) -> Guard {
        loop {
            let now = Instant::now();

            let deadline = {
                let guard = self.deadline.lock().await;

                match &*guard {
                    None => return Guard(guard),
                    Some(deadline) if now >= *deadline => return Guard(guard),
                    Some(deadline) => *deadline,
                }
            };

            sleep_until(deadline).await;
        }
    }

    pub async fn check<F, R>(self, fut: F) -> R
    where
        F: Future<Output = R>,
    {
        let guard = self.lock().await;
        let result = fut.await;
        drop(guard);
        result
    }
}
