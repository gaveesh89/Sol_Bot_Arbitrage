use backoff::{ExponentialBackoff, Error as BackoffError};
use std::time::Duration;
use tracing::warn;

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_interval: Duration,
    pub max_interval: Duration,
    pub multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    pub fn to_exponential_backoff(&self) -> ExponentialBackoff {
        ExponentialBackoff {
            initial_interval: self.initial_interval,
            max_interval: self.max_interval,
            multiplier: self.multiplier,
            max_elapsed_time: Some(Duration::from_secs(30)),
            ..Default::default()
        }
    }

    /// Retry an async operation with exponential backoff
    pub async fn retry_async<F, Fut, T, E>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let backoff = self.to_exponential_backoff();

        let retry_operation = || async {
            match operation().await {
                Ok(result) => Ok(result),
                Err(e) => {
                    warn!("Operation failed, will retry: {}", e);
                    Err(BackoffError::transient(e))
                }
            }
        };

        backoff::future::retry(backoff, retry_operation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
    }
}
