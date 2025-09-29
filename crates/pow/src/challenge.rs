use std::array::TryFromSliceError;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::{ChallengeError, Domain, Requestor};

/// A challenge for proof-of-work validation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Challenge {
    /// The target used to validate the challenge solution. A lower target makes the challenge more
    /// difficult to solve. A solution is valid if the hash `H(challenge, nonce)`, interpreted as a
    /// big-endian u64 from the first 8 bytes, is less than this target value.
    pub(crate) target: u64,
    /// The timestamp of the challenge creation.
    pub(crate) timestamp: u64,
    /// The requestor of the challenge.
    pub(crate) requestor: Requestor,
    /// The domain used to request the challenge.
    pub(crate) domain: Domain,
    /// Deterministically generated signature of the challenge.
    pub(crate) signature: [u8; 32],
}

impl Challenge {
    /// The size of the serialized challenge in bytes.
    pub const SERIALIZED_SIZE: usize = 112;

    /// Creates a new challenge with the given parameters.
    /// The signature is computed internally using the provided secret.
    pub fn new(
        target: u64,
        timestamp: u64,
        requestor: Requestor,
        domain: Domain,
        secret: [u8; 32],
    ) -> Self {
        let signature = Self::compute_signature(secret, target, timestamp, &requestor, &domain);
        Self {
            target,
            timestamp,
            requestor,
            domain,
            signature,
        }
    }

    /// Creates a challenge from existing parts (used for decoding).
    pub fn from_parts(
        target: u64,
        timestamp: u64,
        requestor: Requestor,
        domain: Domain,
        signature: [u8; 32],
    ) -> Self {
        Self {
            target,
            timestamp,
            requestor,
            domain,
            signature,
        }
    }

    /// Decodes the challenge and verifies that the signature part of the challenge is valid
    /// in the context of the specified secret.
    pub fn verify_signature(&self, secret: [u8; 32]) -> Result<(), ChallengeError> {
        if self.signature
            == Self::compute_signature(
                secret,
                self.target,
                self.timestamp,
                &self.requestor,
                &self.domain,
            )
        {
            Ok(())
        } else {
            Err(ChallengeError::ServerSignaturesDoNotMatch)
        }
    }

    /// Checks whether the provided nonce satisfies the target requirement encoded in the
    /// challenge.
    ///
    /// The solution is valid if the hash `H(challenge, nonce)`, interpreted as a
    /// big-endian u64 from the first 8 bytes, is lower than the target value.
    pub fn validate_pow(&self, nonce: u64) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        hasher.update(nonce.to_be_bytes());
        let hash = hasher.finalize();
        // take 8 bytes from the hash and parse them as u64
        let number = u64::from_be_bytes(hash[..8].try_into().unwrap());

        number < self.target
    }

    /// Checks if the challenge timestamp is expired.
    ///
    /// # Arguments
    /// * `current_time` - The current timestamp in seconds since the UNIX epoch.
    /// * `challenge_lifetime` - The duration during which a challenge is valid.
    pub fn is_expired(&self, current_time: u64, challenge_lifetime: Duration) -> bool {
        let diff = current_time.checked_sub(self.timestamp).unwrap_or(u64::MAX);
        diff > challenge_lifetime.as_secs()
    }

    /// Computes the signature for a challenge.
    pub fn compute_signature(
        secret: [u8; 32],
        target: u64,
        timestamp: u64,
        requestor: &Requestor,
        domain: &Domain,
    ) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(secret);
        hasher.update(target.to_le_bytes());
        hasher.update(timestamp.to_le_bytes());
        hasher.update(requestor);
        hasher.update(domain);
        hasher.finalize().into()
    }
}

// SERIALIZATION
// ================================================================================================

impl Challenge {
    pub fn to_bytes(&self) -> [u8; Self::SERIALIZED_SIZE] {
        let mut bytes = [0u8; Self::SERIALIZED_SIZE];
        bytes[..8].copy_from_slice(&self.target.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.timestamp.to_le_bytes());
        bytes[16..48].copy_from_slice(&self.requestor);
        bytes[48..80].copy_from_slice(&self.domain);
        bytes[80..].copy_from_slice(&self.signature);
        bytes
    }
}

impl From<Challenge> for [u8; Challenge::SERIALIZED_SIZE] {
    fn from(challenge: Challenge) -> Self {
        challenge.to_bytes()
    }
}

impl TryFrom<[u8; Challenge::SERIALIZED_SIZE]> for Challenge {
    type Error = TryFromSliceError;
    fn try_from(bytes: [u8; Challenge::SERIALIZED_SIZE]) -> Result<Self, Self::Error> {
        Ok(Self::from_parts(
            u64::from_le_bytes(bytes[..8].try_into()?),
            u64::from_le_bytes(bytes[8..16].try_into()?),
            bytes[16..48].try_into()?,
            bytes[48..80].try_into()?,
            bytes[80..].try_into()?,
        ))
    }
}

impl TryFrom<&[u8]> for Challenge {
    type Error = ChallengeError;
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let array: [u8; Challenge::SERIALIZED_SIZE] =
            bytes.try_into().map_err(|_| ChallengeError::InvalidSerialization)?;
        Challenge::try_from(array).map_err(|_| ChallengeError::InvalidSerialization)
    }
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn create_test_secret() -> [u8; 32] {
        let mut secret = [0u8; 32];
        secret[..12].copy_from_slice(b"miden-faucet");
        secret
    }

    #[test]
    fn challenge_serialize_and_deserialize() {
        let secret = [1u8; 32];
        let requestor = [1u8; 32];
        let domain = [2u8; 32];
        let challenge = Challenge::new(2, 1_234_567_890, requestor, domain, secret);

        let serialized = challenge.to_bytes();

        let deserialized = Challenge::try_from(serialized).unwrap();
        assert_eq!(deserialized, challenge);
    }

    #[test]
    fn timestamp_validation() {
        let secret = create_test_secret();
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let requestor = [1u8; 32];
        let domain = [2u8; 32];
        let challenge_lifetime = Duration::from_secs(30);

        // Valid timestamp (current time)
        let challenge = Challenge::new(12, current_time, requestor, domain, secret);
        assert!(!challenge.is_expired(current_time, challenge_lifetime));

        // Expired timestamp (too old)
        let old_timestamp = current_time - challenge_lifetime.as_secs() - 10;
        let challenge = Challenge::new(12, old_timestamp, requestor, domain, secret);
        assert!(challenge.is_expired(current_time, challenge_lifetime));
    }
}
