use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use miden_client::utils::Deserializable;
use tokio::time::Duration;

use crate::challenge_cache::ChallengeCache;

mod challenge;
mod challenge_cache;

pub use challenge::Challenge;

// PoW Rate Limiter
// ================================================================================================

/// Proof-of-Work Rate Limiter implementation.
///
/// This struct is used to enforce rate limiting based on `PoW` challenges.
#[derive(Clone)]
pub struct PoWRateLimiter {
    /// The server secret used to sign and validate challenges.
    secret: [u8; 32],
    /// The cache used to store submitted challenges.
    challenges: Arc<Mutex<ChallengeCache>>,
    /// The settings of the rate limiter.
    config: PoWRateLimiterConfig,
}

/// Represents the requestor of a challenge.
type Requestor = [u8; 32];

/// Represents the domain of a challenge, which is a unique identifier for the service that is
/// requesting a challenge.
type Domain = [u8; 32];

/// The settings of `PoWRateLimiter`.
#[derive(Clone)]
pub struct PoWRateLimiterConfig {
    /// The lifetime for challenges. After this time, challenges are considered expired.
    pub challenge_lifetime: Duration,
    /// Determines how much the difficulty increases with the amount of active challenges. The
    /// difficulty is computed as `num_active_challenges << growth_rate`.
    pub growth_rate: NonZeroUsize,
    /// Sets the `max_target` used for challenges. The initial target (with difficulty = 1) for
    /// challenges will be `u64::MAX >> baseline`.
    pub baseline: u8,
    /// The interval at which the challenge cache is cleaned up. Only expired challenges are
    /// removed during cleanup.
    pub cleanup_interval: Duration,
}

impl PoWRateLimiter {
    /// Creates a new `PoW` instance.
    pub fn new(secret: [u8; 32], config: PoWRateLimiterConfig) -> Self {
        let challenge_cache = Arc::new(Mutex::new(ChallengeCache::default()));

        // Start the cleanup task
        let cleanup_state = challenge_cache.clone();
        tokio::spawn(async move {
            ChallengeCache::run_cleanup(
                cleanup_state,
                config.challenge_lifetime,
                config.cleanup_interval,
            )
            .await;
        });

        Self {
            secret,
            challenges: challenge_cache,
            config,
        }
    }

    /// Generates a new challenge.
    pub fn build_challenge(
        &self,
        requestor: impl Into<Requestor>,
        domain: impl Into<Domain>,
    ) -> Challenge {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current timestamp should be greater than unix epoch")
            .as_secs();
        let requestor = requestor.into();
        let domain = domain.into();
        let target = self.get_challenge_target(&domain);

        Challenge::new(target, current_time, requestor, domain, self.secret)
    }

    /// Computes the target for a given domain by checking the amount of active challenges in the
    /// cache. This sets the difficulty of the challenge.
    ///
    /// It is computed as:
    /// `max_target / difficulty`
    ///
    /// Where:
    /// * `max_target = u64::MAX >> baseline`
    /// * `difficulty = max(num_active_challenges << growth_rate, 1)`
    fn get_challenge_target(&self, domain: &Domain) -> u64 {
        let num_challenges = self
            .challenges
            .lock()
            .expect("challenge cache lock should not be poisoned")
            .num_challenges_for_domain(domain);

        let max_target = u64::MAX >> self.config.baseline;
        let difficulty = usize::max(num_challenges << self.config.growth_rate.get(), 1);
        max_target / difficulty as u64
    }

    /// Submits a challenge.
    ///
    /// The challenge is validated and added to the cache.
    ///
    /// # Errors
    /// Returns an error if:
    /// * The challenge is expired.
    /// * The challenge is invalid.
    /// * The challenge was already used.
    /// * The requestor has already submitted a challenge recently and it's not expired yet.
    ///
    /// # Panics
    /// Panics if the challenge cache lock is poisoned.
    pub fn submit_challenge(
        &self,
        requestor: impl Into<Requestor>,
        domain: impl Into<Domain>,
        challenge_bytes: &[u8],
        nonce: u64,
        current_time: u64,
    ) -> Result<(), ChallengeError> {
        let challenge = Challenge::read_from_bytes(challenge_bytes)
            .map_err(|_| ChallengeError::InvalidSerialization)?;
        challenge.verify_signature(self.secret)?;
        let requestor = requestor.into();
        let domain = domain.into();

        // Check timestamp validity
        if challenge.is_expired(current_time, self.config.challenge_lifetime) {
            return Err(ChallengeError::ExpiredServerTimestamp(challenge.timestamp, current_time));
        }

        // Validate the challenge
        let valid_requestor = requestor == challenge.requestor;
        let valid_domain = domain == challenge.domain;
        let valid_nonce = challenge.validate_pow(nonce);
        if !(valid_nonce && valid_requestor && valid_domain) {
            return Err(ChallengeError::InvalidPoW);
        }

        let mut challenge_cache =
            self.challenges.lock().expect("challenge cache lock should not be poisoned");

        // Check if requestor has recently submitted a challenge.
        if challenge_cache.has_challenge_for_requestor(requestor) {
            return Err(ChallengeError::RateLimited);
        }

        // Check if the cache already contains the challenge. If not, it is inserted.
        if !challenge_cache.insert_challenge(&challenge) {
            return Err(ChallengeError::ChallengeAlreadyUsed);
        }

        Ok(())
    }
}

/// `PoW` challenge related errors.
#[derive(Debug, thiserror::Error)]
pub enum ChallengeError {
    #[error("server timestamp expired, received: {0}, current time: {1}")]
    ExpiredServerTimestamp(u64, u64),
    #[error("invalid PoW solution")]
    InvalidPoW,
    #[error("requestor is rate limited")]
    RateLimited,
    #[error("challenge already used")]
    ChallengeAlreadyUsed,
    #[error("server signatures do not match")]
    ServerSignaturesDoNotMatch,
    #[error("invalid challenge serialization")]
    InvalidSerialization,
    #[error("domain {0} is invalid")]
    InvalidDomain(String),
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use miden_client::utils::Serializable;

    use super::*;

    fn find_pow_solution(challenge: &Challenge, max_iterations: u64) -> Option<u64> {
        (0..max_iterations).find(|&nonce| challenge.validate_pow(nonce))
    }

    fn create_test_pow() -> PoWRateLimiter {
        let mut secret = [0u8; 32];
        secret[..12].copy_from_slice(b"miden-faucet");

        PoWRateLimiter::new(
            secret,
            PoWRateLimiterConfig {
                challenge_lifetime: Duration::from_secs(30),
                cleanup_interval: Duration::from_millis(500),
                growth_rate: NonZeroUsize::new(2).unwrap(),
                baseline: 0,
            },
        )
    }

    #[tokio::test]
    async fn test_pow_validation() {
        let pow = create_test_pow();
        let domain = [1u8; 32];
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let requestor = [0u8; 32];
        let challenge = pow.build_challenge(requestor, domain);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        // Submit challenge with correct nonce - should succeed
        let result =
            pow.submit_challenge(requestor, domain, &challenge.to_bytes(), nonce, current_time);
        assert!(result.is_ok());

        // Try to use the same challenge again with another requestor - should fail
        let requestor = [1u8; 32];
        let result =
            pow.submit_challenge(requestor, domain, &challenge.to_bytes(), nonce, current_time);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timestamp_validation() {
        let pow = create_test_pow();
        let domain = [1u8; 32];
        let requestor = [0u8; 32];
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let challenge = pow.build_challenge(requestor, domain);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        // Submit challenge with expired timestamp - should fail
        let result = pow.submit_challenge(
            requestor,
            domain,
            &challenge.to_bytes(),
            nonce,
            current_time + pow.config.challenge_lifetime.as_secs() + 1,
        );
        assert!(result.is_err());

        // Submit challenge with correct timestamp - should succeed
        let result =
            pow.submit_challenge(requestor, domain, &challenge.to_bytes(), nonce, current_time);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn requestor_is_rate_limited() {
        let pow = create_test_pow();
        let domain = [1u8; 32];
        let requestor = [0u8; 32];

        // Solve first challenge
        let challenge = pow.build_challenge(requestor, domain);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let result =
            pow.submit_challenge(requestor, domain, &challenge.to_bytes(), nonce, current_time);
        assert!(result.is_ok());

        // Try to submit second challenge - should fail because of rate limiting
        tokio::time::sleep(pow.config.cleanup_interval).await;
        let challenge = pow.build_challenge(requestor, domain);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let result =
            pow.submit_challenge(requestor, domain, &challenge.to_bytes(), nonce, current_time);
        assert!(result.is_err());
        assert!(matches!(result.err(), Some(ChallengeError::RateLimited)));
    }

    #[tokio::test]
    async fn submit_challenge_and_check_difficulty() {
        let mut pow = create_test_pow();
        pow.config.growth_rate = NonZeroUsize::new(1).unwrap();
        let domain = [1u8; 32];
        let requestor = [0u8; 32];
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        assert_eq!(pow.get_challenge_target(&domain), u64::MAX >> pow.config.baseline);

        let challenge = pow.build_challenge(requestor, domain);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        pow.submit_challenge(requestor, domain, &challenge.to_bytes(), nonce, current_time)
            .unwrap();

        assert_eq!(pow.challenges.lock().unwrap().num_challenges_for_domain(&domain), 1);
        assert_eq!(pow.get_challenge_target(&domain), (u64::MAX >> pow.config.baseline) / 2);
    }

    #[tokio::test]
    async fn test_cleanup_expired_challenges() {
        let pow = create_test_pow();
        let domain = [1u8; 32];
        let requestor = [0u8; 32];
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let target = u64::MAX;

        // build challenge manually with past timestamp to ensure that expires in 1 second
        let timestamp = current_time - pow.config.challenge_lifetime.as_secs();
        let signature =
            Challenge::compute_signature(pow.secret, target, timestamp, &requestor, &domain);
        let challenge = Challenge::from_parts(target, timestamp, requestor, domain, signature);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        pow.submit_challenge(requestor, domain, &challenge.to_bytes(), nonce, current_time)
            .unwrap();

        // wait for cleanup
        tokio::time::sleep(pow.config.cleanup_interval + Duration::from_secs(1)).await;

        // check that the challenge is removed from the cache
        assert!(!pow.challenges.lock().unwrap().has_challenge_for_requestor(requestor));
        assert_eq!(pow.challenges.lock().unwrap().num_challenges_for_domain(&domain), 0);

        // submit second challenge - should succeed
        let challenge = pow.build_challenge(requestor, domain);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        pow.submit_challenge(requestor, domain, &challenge.to_bytes(), nonce, current_time)
            .unwrap();

        assert!(pow.challenges.lock().unwrap().has_challenge_for_requestor(requestor));
        assert_eq!(pow.challenges.lock().unwrap().num_challenges_for_domain(&domain), 1);
    }
}
