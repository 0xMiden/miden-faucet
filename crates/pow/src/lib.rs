use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use miden_client::account::AccountId;
use tokio::time::Duration;

use crate::challenge_cache::ChallengeCache;

mod api_key;
mod challenge;
mod challenge_cache;

pub use api_key::ApiKey;
pub use challenge::Challenge;

// POW
// ================================================================================================

/// Proof-of-Work implementation.
///
/// This struct is used to generate and validate `PoW` challenges.
#[derive(Clone)]
pub struct PoW {
    /// The server secret used to sign and validate challenges.
    secret: [u8; 32],
    /// The cache used to store submitted challenges.
    challenge_cache: Arc<Mutex<ChallengeCache>>,
    /// The configuration settings.
    config: PoWConfig,
}

/// The configuration settings for `PoW`.
#[derive(Clone)]
pub struct PoWConfig {
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

impl PoW {
    /// Creates a new `PoW` instance.
    pub fn new(secret: [u8; 32], config: PoWConfig) -> Self {
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

        Self { secret, challenge_cache, config }
    }

    /// Generates a new challenge.
    pub fn build_challenge(&self, account_id: AccountId, api_key: ApiKey) -> Challenge {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current timestamp should be greater than unix epoch")
            .as_secs();
        let target = self.get_challenge_target(&api_key);

        Challenge::new(target, current_time, account_id, api_key, self.secret)
    }

    /// Computes the target for a given API key by checking the amount of active challenges in the
    /// cache. This sets the difficulty of the challenge.
    ///
    /// It is computed as:
    /// `max_target / difficulty`
    ///
    /// Where:
    /// * `max_target = u64::MAX >> baseline`
    /// * `difficulty = max(num_active_challenges << growth_rate, 1)`
    fn get_challenge_target(&self, api_key: &ApiKey) -> u64 {
        let num_challenges = self
            .challenge_cache
            .lock()
            .expect("challenge cache lock should not be poisoned")
            .num_challenges_for_api_key(api_key);

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
    /// * The account has already submitted a challenge recently and it's not expired yet.
    ///
    /// # Panics
    /// Panics if the challenge cache lock is poisoned.
    pub fn submit_challenge(
        &self,
        account_id: AccountId,
        api_key: &ApiKey,
        challenge: &str,
        nonce: u64,
        current_time: u64,
    ) -> Result<(), PowError> {
        let challenge = Challenge::decode(challenge, self.secret)?;

        // Check timestamp validity
        if challenge.is_expired(current_time, self.config.challenge_lifetime) {
            return Err(PowError::ExpiredServerTimestamp(challenge.timestamp, current_time));
        }

        // Validate the challenge
        let valid_account_id = account_id == challenge.account_id;
        let valid_api_key = *api_key == challenge.api_key;
        let valid_nonce = challenge.validate_pow(nonce);
        if !(valid_nonce && valid_account_id && valid_api_key) {
            return Err(PowError::InvalidPoW);
        }

        let mut challenge_cache = self
            .challenge_cache
            .lock()
            .expect("challenge cache lock should not be poisoned");

        // Check if account has recently submitted a challenge.
        if let Some(timestamp) = challenge_cache.has_challenge(account_id, api_key.clone()) {
            return Err(PowError::RateLimited(
                timestamp
                    + self.config.challenge_lifetime.as_secs()
                    + self.config.cleanup_interval.as_secs(),
            ));
        }

        // Check if the cache already contains the challenge. If not, it is inserted.
        if !challenge_cache.insert_challenge(&challenge) {
            return Err(PowError::ChallengeAlreadyUsed);
        }

        Ok(())
    }
}

/// `PoW` challenge related errors.
#[derive(Debug, thiserror::Error)]
pub enum PowError {
    #[error("server timestamp expired, received: {0}, current time: {1}")]
    ExpiredServerTimestamp(u64, u64),
    #[error("invalid POW solution")]
    InvalidPoW,
    #[error("account is rate limited")]
    RateLimited(u64),
    #[error("challenge already used")]
    ChallengeAlreadyUsed,
    #[error("server signatures do not match")]
    ServerSignaturesDoNotMatch,
    #[error("invalid challenge")]
    InvalidChallenge,
    #[error("API key {0} is invalid")]
    InvalidApiKey(String),
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;

    use super::*;

    fn find_pow_solution(challenge: &Challenge, max_iterations: u64) -> Option<u64> {
        (0..max_iterations).find(|&nonce| challenge.validate_pow(nonce))
    }

    fn create_test_pow() -> PoW {
        let mut secret = [0u8; 32];
        secret[..12].copy_from_slice(b"miden-faucet");

        PoW::new(
            secret,
            PoWConfig {
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
        let mut rng = ChaCha20Rng::from_seed(rand::random());
        let api_key = ApiKey::generate(&mut rng);
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let account_id = 0_u128.try_into().unwrap();
        let challenge = pow.build_challenge(account_id, api_key.clone());
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        // Submit challenge with correct nonce - should succeed
        let result =
            pow.submit_challenge(account_id, &api_key, &challenge.encode(), nonce, current_time);
        assert!(result.is_ok());

        // Try to use the same challenge again with another account - should fail
        let account_id = 1_u128.try_into().unwrap();
        let result =
            pow.submit_challenge(account_id, &api_key, &challenge.encode(), nonce, current_time);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timestamp_validation() {
        let pow = create_test_pow();
        let mut rng = ChaCha20Rng::from_seed(rand::random());
        let api_key = ApiKey::generate(&mut rng);
        let account_id = [0u8; AccountId::SERIALIZED_SIZE].try_into().unwrap();
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let challenge = pow.build_challenge(account_id, api_key.clone());
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        // Submit challenge with expired timestamp - should fail
        let result = pow.submit_challenge(
            account_id,
            &api_key,
            &challenge.encode(),
            nonce,
            current_time + pow.config.challenge_lifetime.as_secs() + 1,
        );
        assert!(result.is_err());

        // Submit challenge with correct timestamp - should succeed
        let result =
            pow.submit_challenge(account_id, &api_key, &challenge.encode(), nonce, current_time);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn account_id_is_rate_limited() {
        let pow = create_test_pow();
        let mut rng = ChaCha20Rng::from_seed(rand::random());
        let api_key = ApiKey::generate(&mut rng);
        let account_id = [0u8; AccountId::SERIALIZED_SIZE].try_into().unwrap();

        // Solve first challenge
        let challenge = pow.build_challenge(account_id, api_key.clone());
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let result =
            pow.submit_challenge(account_id, &api_key, &challenge.encode(), nonce, current_time);
        assert!(result.is_ok());

        // Try to submit second challenge - should fail because of rate limiting
        tokio::time::sleep(pow.config.cleanup_interval).await;
        let challenge = pow.build_challenge(account_id, api_key.clone());
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        let second_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let result =
            pow.submit_challenge(account_id, &api_key, &challenge.encode(), nonce, second_time);
        assert!(result.is_err());
        let expected_timestamp = current_time
            + pow.config.challenge_lifetime.as_secs()
            + pow.config.cleanup_interval.as_secs();
        match result {
            Err(PowError::RateLimited(timestamp)) => assert_eq!(timestamp, expected_timestamp),
            _ => panic!("Expected RateLimited error"),
        }
    }

    #[tokio::test]
    async fn submit_challenge_and_check_difficulty() {
        let mut pow = create_test_pow();
        pow.config.growth_rate = NonZeroUsize::new(1).unwrap();
        let mut rng = ChaCha20Rng::from_seed(rand::random());
        let api_key = ApiKey::generate(&mut rng);
        let account_id = [0u8; AccountId::SERIALIZED_SIZE].try_into().unwrap();
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        assert_eq!(pow.get_challenge_target(&api_key), u64::MAX >> pow.config.baseline);

        let challenge = pow.build_challenge(account_id, api_key.clone());
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        pow.submit_challenge(account_id, &api_key, &challenge.encode(), nonce, current_time)
            .unwrap();

        assert_eq!(pow.challenge_cache.lock().unwrap().num_challenges_for_api_key(&api_key), 1);
        assert_eq!(pow.get_challenge_target(&api_key), (u64::MAX >> pow.config.baseline) / 2);
    }

    #[tokio::test]
    async fn test_cleanup_expired_challenges() {
        let pow = create_test_pow();
        let mut rng = ChaCha20Rng::from_seed(rand::random());
        let api_key = ApiKey::generate(&mut rng);
        let account_id = [0u8; AccountId::SERIALIZED_SIZE].try_into().unwrap();
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let target = u64::MAX;

        // build challenge manually with past timestamp to ensure that expires in 1 second
        let timestamp = current_time - pow.config.challenge_lifetime.as_secs();
        let signature = Challenge::compute_signature(
            pow.secret,
            target,
            timestamp,
            account_id,
            &api_key.inner(),
        );
        let challenge =
            Challenge::from_parts(target, timestamp, account_id, api_key.clone(), signature);
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        pow.submit_challenge(account_id, &api_key, &challenge.encode(), nonce, current_time)
            .unwrap();

        // wait for cleanup
        tokio::time::sleep(pow.config.cleanup_interval + Duration::from_secs(1)).await;

        // check that the challenge is removed from the cache
        assert!(
            pow.challenge_cache
                .lock()
                .unwrap()
                .has_challenge(account_id, api_key.clone())
                .is_none()
        );
        assert_eq!(pow.challenge_cache.lock().unwrap().num_challenges_for_api_key(&api_key), 0);

        // submit second challenge - should succeed
        let challenge = pow.build_challenge(account_id, api_key.clone());
        let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        pow.submit_challenge(account_id, &api_key, &challenge.encode(), nonce, current_time)
            .unwrap();

        assert!(
            pow.challenge_cache
                .lock()
                .unwrap()
                .has_challenge(account_id, api_key.clone())
                .is_some()
        );
        assert_eq!(pow.challenge_cache.lock().unwrap().num_challenges_for_api_key(&api_key), 1);
    }
}
