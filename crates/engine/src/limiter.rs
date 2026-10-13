use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use clickman_storage::RateLimit;

const MAX_BUCKETS: usize = 100_000;
const IDLE: Duration = Duration::from_secs(600);
const PRUNE_INTERVAL: Duration = Duration::from_secs(30);
const MAX_RETRY_SECONDS: u64 = 3600;

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    Limited { retry_after: Duration },
}

/// A token bucket per client: `burst` requests at once, refilled at
/// `per_second`.
#[derive(Default)]
pub struct RateLimiter {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    buckets: HashMap<String, Bucket>,
    last_pruned: Option<Instant>,
}

struct Bucket {
    tokens: f64,
    refreshed: Instant,
}

impl RateLimiter {
    pub fn check(&self, client: &str, limit: RateLimit, now: Instant) -> Decision {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if !state.buckets.contains_key(client) && state.buckets.len() >= MAX_BUCKETS {
            // Bound memory and avoid rescanning 100k live buckets for every new
            // address. Existing clients keep their buckets during a spike.
            if state
                .last_pruned
                .is_none_or(|last| now.saturating_duration_since(last) >= PRUNE_INTERVAL)
            {
                state
                    .buckets
                    .retain(|_, bucket| now.saturating_duration_since(bucket.refreshed) < IDLE);
                state.last_pruned = Some(now);
            }
            if state.buckets.len() >= MAX_BUCKETS {
                return Decision::Limited { retry_after: IDLE };
            }
        }

        let bucket = state.buckets.entry(client.to_owned()).or_insert(Bucket {
            tokens: limit.burst,
            refreshed: now,
        });
        let elapsed = now
            .saturating_duration_since(bucket.refreshed)
            .as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * limit.per_second).min(limit.burst);
        bucket.refreshed = bucket.refreshed.max(now);

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Decision::Allowed
        } else {
            // Tiny valid refill rates may produce infinity or exceed Duration's
            // representation. A bounded retry remains useful to durable SDKs.
            let seconds = ((1.0 - bucket.tokens) / limit.per_second)
                .ceil()
                .min(MAX_RETRY_SECONDS as f64)
                .max(1.0) as u64;
            Decision::Limited {
                retry_after: Duration::from_secs(seconds),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: RateLimit = RateLimit {
        per_second: 2.0,
        burst: 3.0,
    };

    #[test]
    fn a_burst_is_allowed_then_the_client_waits_for_a_refill() {
        let limiter = RateLimiter::default();
        let start = Instant::now();

        for _ in 0..3 {
            assert_eq!(limiter.check("a", LIMIT, start), Decision::Allowed);
        }
        assert_eq!(
            limiter.check("a", LIMIT, start),
            Decision::Limited {
                retry_after: Duration::from_secs(1)
            }
        );
        assert_eq!(
            limiter.check("a", LIMIT, start + Duration::from_millis(500)),
            Decision::Allowed
        );
    }

    #[test]
    fn clients_have_their_own_buckets() {
        let limiter = RateLimiter::default();
        let now = Instant::now();

        for _ in 0..3 {
            limiter.check("a", LIMIT, now);
        }

        assert_eq!(limiter.check("b", LIMIT, now), Decision::Allowed);
    }

    #[test]
    fn a_bucket_never_holds_more_than_the_burst() {
        let limiter = RateLimiter::default();
        let now = Instant::now();
        limiter.check("a", LIMIT, now);

        let later = now + Duration::from_secs(60);
        for _ in 0..3 {
            assert_eq!(limiter.check("a", LIMIT, later), Decision::Allowed);
        }
        assert!(matches!(
            limiter.check("a", LIMIT, later),
            Decision::Limited { .. }
        ));
    }

    #[test]
    fn overlapping_requests_can_reach_the_bucket_out_of_timestamp_order() {
        let limiter = RateLimiter::default();
        let first = Instant::now();
        let later = first + Duration::from_secs(1);
        assert_eq!(limiter.check("a", LIMIT, later), Decision::Allowed);
        // Another request captured Instant before waiting for the mutex.
        assert_eq!(limiter.check("a", LIMIT, first), Decision::Allowed);
        assert_eq!(limiter.check("a", LIMIT, later), Decision::Allowed);
        assert!(matches!(
            limiter.check("a", LIMIT, later),
            Decision::Limited { .. }
        ));
    }

    #[test]
    fn extremely_small_valid_rates_return_a_bounded_retry_instead_of_panicking() {
        let limiter = RateLimiter::default();
        let now = Instant::now();
        let limit = RateLimit {
            per_second: f64::MIN_POSITIVE / 2.0,
            burst: 1.0,
        };
        assert_eq!(limiter.check("a", limit, now), Decision::Allowed);
        let Decision::Limited { retry_after } = limiter.check("a", limit, now) else {
            panic!("empty bucket was allowed")
        };
        assert!((Duration::from_secs(1)..=Duration::from_secs(3600)).contains(&retry_after));
    }

    #[test]
    fn client_capacity_is_bounded_and_idle_slots_are_reused() {
        let limiter = RateLimiter::default();
        let now = Instant::now();
        {
            let mut state = limiter.state.lock().unwrap();
            state.buckets.extend((0..MAX_BUCKETS).map(|index| {
                (
                    format!("client-{index}"),
                    Bucket {
                        tokens: 1.0,
                        refreshed: now,
                    },
                )
            }));
        }
        assert_eq!(
            limiter.check("new", LIMIT, now),
            Decision::Limited { retry_after: IDLE }
        );
        assert_eq!(limiter.state.lock().unwrap().buckets.len(), MAX_BUCKETS);
        assert_eq!(limiter.check("client-0", LIMIT, now), Decision::Allowed);
        assert_eq!(limiter.check("new", LIMIT, now + IDLE), Decision::Allowed);
        assert_eq!(limiter.state.lock().unwrap().buckets.len(), 1);
    }
}
