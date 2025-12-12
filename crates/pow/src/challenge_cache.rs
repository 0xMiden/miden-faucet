use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use crate::challenge::Challenge;
use crate::{ChallengeError, Domain, Requestor};

/// Represents the solver of a challenge, i.e. a requestor and a domain.
pub(crate) type Solver = (Requestor, Domain);

// CHALLENGE CACHE
// ================================================================================================

/// A cache that keeps track of the submitted challenges.
///
/// The cache is used to check if a challenge has already been submitted for a given requestor and
/// domain. It also keeps track of the number of challenges submitted for each domain.
///
/// The cache is cleaned up periodically, removing expired challenges.
#[derive(Clone)]
pub(crate) struct ChallengeCache {
    /// The lifetime for challenges. After this time, challenges are considered expired.
    challenge_lifetime: Duration,
    /// Maps challenge timestamp to solvers. We use this to cleanup expired challenges and update
    /// the solvers' last challenge timestamp.
    challenges: BTreeMap<u64, HashSet<Solver>>,
    /// Maps domain to the number of submitted challenges. We use this to compute the load
    /// difficulty.
    challenges_per_domain: HashMap<Domain, usize>,
    /// Maps solver to the timestamp of the last submitted challenge. We use this to check if
    /// solvers can submit new challenges.
    challenges_timestamps: HashMap<Solver, u64>,
}

impl ChallengeCache {
    /// Creates a new challenge cache with the given challenges lifetime.
    pub fn new(challenge_lifetime: Duration) -> Self {
        Self {
            challenge_lifetime,
            challenges: BTreeMap::new(),
            challenges_per_domain: HashMap::new(),
            challenges_timestamps: HashMap::new(),
        }
    }

    /// Inserts a challenge into the cache.
    ///
    /// # Errors
    /// Returns an error if the solver is rate limited.
    pub fn insert_challenge(
        &mut self,
        challenge: &Challenge,
        current_time: u64,
    ) -> Result<(), ChallengeError> {
        let solver = (challenge.requestor, challenge.domain);

        // Check if the solver is rate limited. There could still be an expired challenge in the
        // cache for this solver, so in that case we override it.
        let remaining_time = self.next_challenge_delay(&solver, current_time);
        if remaining_time != 0 {
            return Err(ChallengeError::RateLimited(remaining_time));
        }

        self.challenges.entry(current_time).or_default().insert(solver);

        let prev_challenge = self.challenges_timestamps.insert(solver, current_time);
        if let Some(prev_timestamp) = prev_challenge {
            assert!(
                prev_timestamp + self.challenge_lifetime.as_secs() <= current_time,
                "previous timestamp should be expired"
            );
            // Since the previous timestamp for this solver is overridden, we can also just clean
            // up that challenge from the cache. The number of challenges for the domain stays
            // unchanged.
            if let Some(solvers) = self.challenges.get_mut(&prev_timestamp) {
                solvers.remove(&solver);
                if solvers.is_empty() {
                    self.challenges.remove(&prev_timestamp);
                }
            }
        } else {
            // If there was no previous timestamp tracked for this solver, the number of
            // challenges for the domain has to be incremented.
            self.challenges_per_domain
                .entry(challenge.domain)
                .and_modify(|c| *c = c.saturating_add(1))
                .or_insert(1);
        }
        Ok(())
    }

    /// Returns the seconds remaining until the next challenge can be submitted for the given
    /// requestor and domain. If the solver has not submitted a challenge yet, or the previous
    /// one expired, 0 is returned.
    fn next_challenge_delay(&self, solver: &Solver, current_time: u64) -> u64 {
        self.challenges_timestamps.get(solver).map_or(0, |timestamp| {
            (timestamp + self.challenge_lifetime.as_secs()).saturating_sub(current_time)
        })
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
    pub fn cleanup_expired_challenges(&mut self, current_time: u64) {
        // Timestamps lower than this are expired. Add 1 since `BTreeMap::split_off` is inclusive.
        let limit_timestamp = current_time - self.challenge_lifetime.as_secs() + 1;

        let valid_challenges = self.challenges.split_off(&limit_timestamp);
        let expired_challenges = std::mem::replace(&mut self.challenges, valid_challenges);

        for solvers in expired_challenges.into_values() {
            for (requestor, domain) in solvers {
                let remove_domain = self
                    .challenges_per_domain
                    .get_mut(&domain)
                    .map(|c| {
                        *c = c.saturating_sub(1);
                        *c == 0
                    })
                    .expect("domain should have a submitted challenges count");
                if remove_domain {
                    self.challenges_per_domain.remove(&domain);
                }

                self.challenges_timestamps
                    .remove(&(requestor, domain))
                    .expect("solver should have a submitted challenge");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use crate::Challenge;
    use crate::challenge_cache::ChallengeCache;

    #[tokio::test]
    async fn expired_challenges_are_cleaned_up() {
        let challenge_lifetime = Duration::from_millis(100);
        let mut cache = ChallengeCache::new(challenge_lifetime);

        let domain = [1u8; 32];
        let requestor = [0u8; 32];
        let signature = [0u8; 32];
        let target = u64::MAX;
        let request_complexity = 1;

        let insertion_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let challenge = Challenge::from_parts(
            target,
            insertion_timestamp,
            request_complexity,
            requestor,
            domain,
            signature,
        );
        cache.insert_challenge(&challenge, insertion_timestamp).unwrap();

        // assert that the challenge was inserted
        assert!(cache.challenges.contains_key(&insertion_timestamp));
        assert_eq!(cache.challenges_per_domain.get(&domain).unwrap(), &1);
        assert_eq!(
            cache.challenges_timestamps.get(&(requestor, domain)).unwrap(),
            &insertion_timestamp
        );

        // wait for expiration + cleanup
        let expiration_time = insertion_timestamp + challenge_lifetime.as_secs();
        cache.cleanup_expired_challenges(expiration_time);

        // assert that the challenge was removed
        assert!(!cache.challenges.contains_key(&insertion_timestamp));
        assert_eq!(cache.challenges_per_domain.get(&domain), None);
        assert_eq!(cache.challenges_timestamps.get(&(requestor, domain)), None);
    }
}
