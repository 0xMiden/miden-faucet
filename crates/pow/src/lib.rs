use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::time::Duration;

use crate::challenge_cache::ChallengeCache;

mod challenge;
mod challenge_cache;
mod utils;

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
    /// Determines how much the difficulty increases with the amount of active challenges.
    pub growth_rate: f64,
    /// Sets the baseline difficulty bits when there are no active challenges.
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

    /// Generates a new challenge with a difficulty that will depend on the number of active
    /// challenges for the given domain and the request complexity.
    ///
    /// # Arguments
    /// * `requestor` - A unique identifier for the user that is requesting the challenge.
    /// * `domain` - A unique identifier for the service that is requesting the challenge.
    /// * `request_complexity` - A measure of the complexity of the request. Must be greater than 0.
    ///
    /// # Panics
    /// Panics if the request complexity is 0.
    pub fn build_challenge(
        &self,
        requestor: impl Into<Requestor>,
        domain: impl Into<Domain>,
        request_complexity: u64,
    ) -> Challenge {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current timestamp should be greater than unix epoch")
            .as_secs();
        let requestor = requestor.into();
        let domain = domain.into();
        let target = self.get_challenge_target(&domain, request_complexity);

        Challenge::new(target, current_time, request_complexity, requestor, domain, self.secret)
    }

    /// Computes the target for a given domain by checking the amount of active challenges in the
    /// cache and the given request complexity.
    ///
    /// The target is computed as:
    /// `(u64::MAX >> baseline) / request_difficulty`
    ///
    /// Where:
    /// * `request_difficulty = load_difficulty * request_complexity`
    /// * `load_difficulty = ceil((num_active_challenges + 1) * growth_rate)`
    fn get_challenge_target(&self, domain: &Domain, request_complexity: u64) -> u64 {
        let num_challenges = self
            .challenges
            .lock()
            .expect("challenge cache lock should not be poisoned")
            .num_challenges_for_domain(domain) as u64;

        let max_target = u64::MAX >> self.config.baseline;
        #[allow(clippy::cast_precision_loss, reason = "num_challenges is smaller than f64::MAX")]
        #[allow(clippy::cast_sign_loss, reason = "growth_rate and num_challenges are positive")]
        let load_difficulty = ((num_challenges + 1) as f64 * self.config.growth_rate).ceil() as u64;
        let request_difficulty = load_difficulty * request_complexity;
        max_target / request_difficulty
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
        challenge: &str,
        nonce: u64,
        current_time: u64,
        request_complexity: u64,
    ) -> Result<(), ChallengeError> {
        let challenge = Challenge::decode(challenge, self.secret)?;
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
        let valid_request_complexity = challenge.request_complexity == request_complexity;
        if !(valid_nonce && valid_requestor && valid_domain && valid_request_complexity) {
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
    #[error("invalid challenge size")]
    InvalidChallengeSize,
    #[error("domain {0} is invalid")]
    InvalidDomain(String),
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
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
                growth_rate: 1.0,
                cleanup_interval: Duration::from_millis(500),
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
        let request_complexity = 1;
        let challenge = pow.build_challenge(requestor, domain, request_complexity);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        // Submit challenge with correct nonce - should succeed
        let result = pow.submit_challenge(
            requestor,
            domain,
            &challenge.encode(),
            nonce,
            current_time,
            request_complexity,
        );
        assert!(result.is_ok());

        // Try to use the same challenge again with another requestor - should fail
        let requestor = [1u8; 32];
        let result = pow.submit_challenge(
            requestor,
            domain,
            &challenge.encode(),
            nonce,
            current_time,
            request_complexity,
        );
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timestamp_validation() {
        let pow = create_test_pow();
        let domain = [1u8; 32];
        let requestor = [0u8; 32];
        let request_complexity = 1;
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let challenge = pow.build_challenge(requestor, domain, request_complexity);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        // Submit challenge with expired timestamp - should fail
        let result = pow.submit_challenge(
            requestor,
            domain,
            &challenge.encode(),
            nonce,
            current_time + pow.config.challenge_lifetime.as_secs() + 1,
            request_complexity,
        );
        assert!(result.is_err());

        // Submit challenge with correct timestamp - should succeed
        let result = pow.submit_challenge(
            requestor,
            domain,
            &challenge.encode(),
            nonce,
            current_time,
            request_complexity,
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn requestor_is_rate_limited() {
        let pow = create_test_pow();
        let domain = [1u8; 32];
        let requestor = [0u8; 32];
        let request_complexity = 1;

        // Solve first challenge
        let challenge = pow.build_challenge(requestor, domain, request_complexity);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let result = pow.submit_challenge(
            requestor,
            domain,
            &challenge.encode(),
            nonce,
            current_time,
            request_complexity,
        );
        assert!(result.is_ok());

        // Try to submit second challenge - should fail because of rate limiting
        tokio::time::sleep(pow.config.cleanup_interval).await;
        let challenge = pow.build_challenge(requestor, domain, request_complexity);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let result = pow.submit_challenge(
            requestor,
            domain,
            &challenge.encode(),
            nonce,
            current_time,
            request_complexity,
        );
        assert!(result.is_err());
        assert!(matches!(result.err(), Some(ChallengeError::RateLimited)));
    }

    #[tokio::test]
    async fn submit_challenge_and_check_difficulty() {
        let pow = create_test_pow();
        let domain = [1u8; 32];
        let requestor = [0u8; 32];
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let request_complexity = 1;

        assert_eq!(
            pow.get_challenge_target(&domain, request_complexity),
            u64::MAX >> pow.config.baseline
        );

        let challenge = pow.build_challenge(requestor, domain, request_complexity);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        pow.submit_challenge(
            requestor,
            domain,
            &challenge.encode(),
            nonce,
            current_time,
            request_complexity,
        )
        .unwrap();

        assert_eq!(pow.challenges.lock().unwrap().num_challenges_for_domain(&domain), 1);
        assert_eq!(
            pow.get_challenge_target(&domain, request_complexity),
            (u64::MAX >> pow.config.baseline) / 2
        );
    }

    #[tokio::test]
    async fn difficulty_increases_with_request_complexity() {
        let pow = create_test_pow();
        let domain = [1u8; 32];

        // test: request complexity 1 should have difficulty 1
        let request_complexity = 1;

        let difficulty = 1;
        assert_eq!(
            pow.get_challenge_target(&domain, request_complexity),
            (u64::MAX >> pow.config.baseline) / difficulty
        );

        // test: request complexity 3 should have difficulty 3
        let request_complexity = 3;

        let difficulty = 3;
        assert_eq!(
            pow.get_challenge_target(&domain, request_complexity),
            (u64::MAX >> pow.config.baseline) / difficulty
        );
    }

    #[tokio::test]
    async fn test_cleanup_expired_challenges() {
        let pow = create_test_pow();
        let domain = [1u8; 32];
        let requestor = [0u8; 32];
        let request_complexity = 1;
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let target = u64::MAX;

        // build challenge manually with past timestamp to ensure that expires in 1 second
        let timestamp = current_time - pow.config.challenge_lifetime.as_secs();
        let signature = Challenge::compute_signature(
            pow.secret,
            target,
            timestamp,
            request_complexity,
            &requestor,
            &domain,
        );
        let challenge = Challenge::from_parts(
            target,
            timestamp,
            request_complexity,
            requestor,
            domain,
            signature,
        );
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        pow.submit_challenge(
            requestor,
            domain,
            &challenge.encode(),
            nonce,
            current_time,
            request_complexity,
        )
        .unwrap();

        // wait for cleanup
        tokio::time::sleep(pow.config.cleanup_interval + Duration::from_secs(1)).await;

        // check that the challenge is removed from the cache
        assert!(!pow.challenges.lock().unwrap().has_challenge_for_requestor(requestor));
        assert_eq!(pow.challenges.lock().unwrap().num_challenges_for_domain(&domain), 0);

        // submit second challenge - should succeed
        let challenge = pow.build_challenge(requestor, domain, request_complexity);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        pow.submit_challenge(
            requestor,
            domain,
            &challenge.encode(),
            nonce,
            current_time,
            request_complexity,
        )
        .unwrap();

        assert!(pow.challenges.lock().unwrap().has_challenge_for_requestor(requestor));
        assert_eq!(pow.challenges.lock().unwrap().num_challenges_for_domain(&domain), 1);
    }
}
