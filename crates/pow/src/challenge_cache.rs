use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use miden_client::account::AccountId;
use tokio::time::{Duration, interval};

use crate::api_key::ApiKey;
use crate::challenge::Challenge;

// CHALLENGE CACHE
// ================================================================================================

/// A cache that keeps track of the submitted challenges.
///
/// The cache is used to check if a challenge has already been submitted for a given account and API
/// key. It also keeps track of the number of challenges submitted for each API key.
///
/// The cache is cleaned up periodically, removing expired challenges.
#[derive(Clone, Default)]
pub(crate) struct ChallengeCache {
    /// Maps challenge timestamp to a tuple of `AccountId` and `ApiKey`.
    challenges: BTreeMap<u64, Vec<(AccountId, ApiKey)>>,
    /// Maps API key to the number of submitted challenges.
    challenges_per_key: HashMap<ApiKey, usize>,
    /// Maps account id and api key to the timestamp of the last submitted challenge.
    challenges_timestamps: HashMap<(AccountId, ApiKey), u64>,
}

impl ChallengeCache {
    /// Inserts a challenge into the cache, updating the number of challenges submitted for the
    /// account and the API key.
    ///
    /// Returns whether the value was newly inserted. That is:
    /// * If the cache did not previously contain this challenge, `true` is returned.
    /// * If the cache already contained this challenge, `false` is returned, and the cache is not
    ///   modified.
    pub fn insert_challenge(&mut self, challenge: &Challenge) -> bool {
        let account_id = challenge.account_id;
        let api_key = challenge.api_key.clone();

        // check if (timestamp, account_id, api_key) is already in the cache
        let issuers = self.challenges.entry(challenge.timestamp).or_default();
        if issuers.iter().any(|(id, key)| id == &account_id && key == &api_key) {
            return false;
        }

        issuers.push((account_id, api_key.clone()));
        self.challenges_per_key
            .entry(api_key.clone())
            .and_modify(|c| *c = c.saturating_add(1))
            .or_insert(1);
        self.challenges_timestamps.insert((account_id, api_key), challenge.timestamp);
        true
    }

    /// Checks if a challenge has been submitted for the given account and api key. If so, returns
    /// the timestamp of the last submitted challenge.
    pub fn has_challenge(&self, account_id: AccountId, api_key: ApiKey) -> Option<u64> {
        self.challenges_timestamps.get(&(account_id, api_key)).copied()
    }

    /// Returns the number of challenges submitted for the given API key.
    pub fn num_challenges_for_api_key(&self, key: &ApiKey) -> usize {
        self.challenges_per_key.get(key).copied().unwrap_or(0)
    }

    /// Cleanup expired challenges and update the number of challenges submitted per API key and
    /// account id.
    ///
    /// # Arguments
    /// * `current_time` - The current timestamp in seconds since the UNIX epoch.
    /// * `challenge_lifetime` - The duration during which a challenge is valid.
    ///
    /// # Panics
    /// Panics if any expired challenge has no corresponding entries on the account or API key maps.
    fn cleanup_expired_challenges(&mut self, current_time: u64, challenge_lifetime: Duration) {
        // Challenges older than this are expired.
        let limit_timestamp = current_time - challenge_lifetime.as_secs();

        let valid_challenges = self.challenges.split_off(&limit_timestamp);
        let expired_challenges = std::mem::replace(&mut self.challenges, valid_challenges);

        for issuers in expired_challenges.into_values() {
            for (account_id, api_key) in issuers {
                let remove_api_key = self
                    .challenges_per_key
                    .get_mut(&api_key)
                    .map(|c| {
                        *c = c.saturating_sub(1);
                        *c == 0
                    })
                    .expect("challenge should have had a key entry");
                if remove_api_key {
                    self.challenges_per_key.remove(&api_key);
                }

                self.challenges_timestamps
                    .remove(&(account_id, api_key))
                    .expect("challenge should have had a timestamp entry");
            }
        }
    }

    /// Run the cleanup task.
    ///
    /// The cleanup task is responsible for removing expired challenges from the cache.
    /// It runs every minute and removes challenges that are no longer valid because of their
    /// timestamp.
    pub async fn run_cleanup(
        cache: Arc<Mutex<Self>>,
        challenge_lifetime: Duration,
        cleanup_interval: Duration,
    ) {
        let mut interval = interval(cleanup_interval);

        loop {
            interval.tick().await;
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("current timestamp should be greater than unix epoch")
                .as_secs();
            cache
                .lock()
                .expect("challenge cache lock should not be poisoned")
                .cleanup_expired_challenges(current_time, challenge_lifetime);
        }
    }
}
