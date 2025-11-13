use backoff::ExponentialBackoff;
use std::time::Duration;

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
    /// TODO: Fix closure lifetime issue - needs refactoring to use Cell/RefCell or different approach
    #[allow(dead_code)]
    pub async fn retry_async<F, Fut, T, E>(&self, _operation: F) -> Result<T, E>
    where
        F: FnMut() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display + Send + Sync + 'static,
    {
        // Placeholder - needs proper implementation
        // The issue is that we can't capture a mutable reference in a closure
        // that outlives the current scope. Options:
        // 1. Use interior mutability (Cell/RefCell)
        // 2. Change API to accept FnOnce instead of FnMut
        // 3. Use a different retry library that handles this better
        unimplemented!("retry_async needs refactoring to handle closure lifetimes properly")
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
