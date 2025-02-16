use governor::{
    clock::QuantaClock,
    middleware::NoOpMiddleware,
    state::keyed::DashMapStateStore,
    Quota, RateLimiter
};
use std::{
    net::SocketAddr,
    num::NonZeroU32,
    sync::Arc,
    time::Duration,
};

/// A rate limiter for managing connection attempts per client IP.
#[derive(Clone)]
pub struct ConnectionRateLimiter {
    /// The underlying rate limiter instance, shared across instances.
    limiter: Arc<RateLimiter<SocketAddr, DashMapStateStore<SocketAddr>, QuantaClock, NoOpMiddleware>>,
}

impl ConnectionRateLimiter {
    /// Creates a new `ConnectionRateLimiter` with a specified rate limit.
    /// 
    /// # Arguments
    /// 
    /// * `per_second` - The maximum number of requests allowed per second.
    /// 
    /// # Panics
    /// 
    /// This function will panic if `per_second` is zero.
    pub fn new(per_second: u32) -> Self {
        let burst_size = NonZeroU32::new(per_second)
            .expect("Rate limit must be greater than 0");
        
        let quota = Quota::with_period(Duration::from_secs(1))
            .unwrap()
            .allow_burst(burst_size);

        Self {
            limiter: Arc::new(RateLimiter::keyed(quota)),
        }
    }

    /// Checks whether a connection from the given `addr` is allowed.
    ///
    /// This method blocks until the client is allowed to make a request,
    /// enforcing the rate limit policy.
    ///
    /// # Arguments
    ///
    /// * `addr` - The socket address of the client attempting a connection.
    ///
    /// # Returns
    ///
    /// Returns `true` when the request is allowed.
    pub async fn check(&self, addr: SocketAddr) -> bool {
        self.limiter.until_key_ready(&addr).await;
        true
    }
}