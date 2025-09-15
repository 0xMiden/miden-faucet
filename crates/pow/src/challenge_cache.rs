use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::time::interval;

use crate::challenge::Challenge;
use crate::{Domain, Requestor};

// CHALLENGE CACHE
// ================================================================================================

/// A cache that keeps track of the submitted challenges.
///
/// The cache is used to check if a challenge has already been submitted for a given requestor and
/// domain. It also keeps track of the number of challenges submitted for each domain.
///
/// The cache is cleaned up periodically, removing expired challenges.
#[derive(Clone, Default)]
pub(crate) struct ChallengeCache {
    /// Maps challenge timestamp to a tuple of `Requestor` and `Domain`.
    challenges: BTreeMap<u64, Vec<(Requestor, Domain)>>,
    /// Maps domain to the number of submitted challenges.
    challenges_per_domain: HashMap<Domain, usize>,
    /// Maps requestor and domain to the timestamp of the last submitted challenge.
    challenges_timestamps: HashMap<(Requestor, Domain), u64>,
}

impl ChallengeCache {
    /// Inserts a challenge into the cache, updating the number of challenges submitted for the
    /// requestor and the domain.
    ///
    /// Returns whether the value was newly inserted. That is:
    /// * If the cache did not previously contain this challenge, `true` is returned.
    /// * If the cache already contained this challenge, `false` is returned, and the cache is not
    ///   modified.
    pub fn insert_challenge(&mut self, challenge: &Challenge) -> bool {
        let requestor = challenge.requestor;
        let domain = challenge.domain;

        // check if (timestamp, requestor, domain) is already in the cache
        let issuers = self.challenges.entry(challenge.timestamp).or_default();
        if issuers.iter().any(|(r, d)| r == &requestor && *d == domain) {
            return false;
        }

        issuers.push((requestor, domain));
        self.challenges_per_domain
            .entry(domain)
            .and_modify(|c| *c = c.saturating_add(1))
            .or_insert(1);
        self.challenges_timestamps.insert((requestor, domain), challenge.timestamp);
        true
    }

    /// Checks if a challenge has been submitted for the given requestor and domain. If so, returns
    /// the timestamp of the last submitted challenge.
    pub fn has_challenge(&self, requestor: Requestor, domain: Domain) -> Option<u64> {
        self.challenges_timestamps.get(&(requestor, domain)).copied()
    }

    /// Returns the number of challenges submitted for the given domain.
    pub fn num_challenges_for_domain(&self, domain: &Domain) -> usize {
        self.challenges_per_domain.get(domain).copied().unwrap_or(0)
    }

    /// Cleanup expired challenges and update the number of challenges submitted per domain and
    /// requestor.
    ///
    /// # Arguments
    /// * `current_time` - The current timestamp in seconds since the UNIX epoch.
    /// * `challenge_lifetime` - The duration during which a challenge is valid.
    ///
    /// # Panics
    /// Panics if any expired challenge has no corresponding entries on the requestor or domain
    /// maps.
    fn cleanup_expired_challenges(&mut self, current_time: u64, challenge_lifetime: Duration) {
        // Challenges older than this are expired.
        let limit_timestamp = current_time - challenge_lifetime.as_secs();

        let valid_challenges = self.challenges.split_off(&limit_timestamp);
        let expired_challenges = std::mem::replace(&mut self.challenges, valid_challenges);

        for issuers in expired_challenges.into_values() {
            for (requestor, domain) in issuers {
                let remove_domain = self
                    .challenges_per_domain
                    .get_mut(&domain)
                    .map(|c| {
                        *c = c.saturating_sub(1);
                        *c == 0
                    })
                    .expect("challenge should have had a domain entry");
                if remove_domain {
                    self.challenges_per_domain.remove(&domain);
                }

                self.challenges_timestamps
                    .remove(&(requestor, domain))
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
