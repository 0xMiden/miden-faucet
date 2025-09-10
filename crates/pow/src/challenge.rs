use std::time::Duration;

use serde::{Serialize, Serializer};
use sha3::{Digest, Sha3_256};

use crate::utils::{bytes_to_hex, hex_to_bytes};
use crate::{Domain, PowError, Requestor};

/// The size of the encoded challenge in bytes.
const CHALLENGE_ENCODED_SIZE: usize = 112;

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

impl Serialize for Challenge {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Challenge", 3)?;
        state.serialize_field("challenge", &self.encode())?;
        state.serialize_field("target", &self.target)?;
        state.serialize_field("timestamp", &self.timestamp)?;
        state.end()
    }
}

impl Challenge {
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
    pub fn decode(value: &str, secret: [u8; 32]) -> Result<Self, PowError> {
        // Parse the hex-encoded challenge string
        let bytes: [u8; CHALLENGE_ENCODED_SIZE] =
            hex_to_bytes(value).ok_or(PowError::InvalidChallenge)?;

        // SAFETY: Length of the bytes is enforced above.
        let target = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let timestamp = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let requestor = bytes[16..48].try_into().unwrap();
        let domain = bytes[48..80].try_into().unwrap();
        let signature = bytes[80..CHALLENGE_ENCODED_SIZE].try_into().unwrap();

        // Verify the signature
        let expected_signature =
            Self::compute_signature(secret, target, timestamp, &requestor, &domain);
        if signature == expected_signature {
            Ok(Self::from_parts(target, timestamp, requestor, domain, signature))
        } else {
            Err(PowError::ServerSignaturesDoNotMatch)
        }
    }

    /// Encodes the challenge into a hex string.
    pub fn encode(&self) -> String {
        let mut bytes = Vec::with_capacity(CHALLENGE_ENCODED_SIZE);
        bytes.extend_from_slice(&self.target.to_le_bytes());
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.requestor);
        bytes.extend_from_slice(&self.domain);
        bytes.extend_from_slice(&self.signature);
        bytes_to_hex(&bytes)
    }

    /// Checks whether the provided nonce satisfies the target requirement encoded in the
    /// challenge.
    ///
    /// The solution is valid if the hash `H(challenge, nonce)`, interpreted as a
    /// big-endian u64 from the first 8 bytes, is lower than the target value.
    pub fn validate_pow(&self, nonce: u64) -> bool {
        let mut hasher = Sha3_256::new();
        hasher.update(self.encode());
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
        let mut hasher = Sha3_256::new();
        hasher.update(secret);
        hasher.update(target.to_le_bytes());
        hasher.update(timestamp.to_le_bytes());
        hasher.update(requestor);
        hasher.update(domain);
        hasher.finalize().into()
    }
}

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
    fn challenge_serialize_and_deserialize_json() {
        let secret = [1u8; 32];
        let requestor = [1u8; 32];
        let domain = [2u8; 32];
        let challenge = Challenge::new(2, 1_234_567_890, requestor, domain, secret);

        // Test that it serializes to the expected JSON format
        let json = serde_json::to_string(&challenge).unwrap();

        // Should contain the expected fields
        assert!(json.contains("\"challenge\":"));
        assert!(json.contains("\"target\":"));
        assert!(json.contains("\"timestamp\":1234567890"));

        // Parse back to verify structure
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("challenge").is_some());
        assert!(parsed.get("target").is_some());
        assert!(parsed.get("timestamp").is_some());
        assert_eq!(parsed["target"], challenge.target);
        assert_eq!(parsed["timestamp"], 1_234_567_890);
    }

    #[test]
    fn challenge_encode_decode() {
        let secret = create_test_secret();
        let target = 3;
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let requestor = [1u8; 32];
        let domain = [2u8; 32];

        let challenge = Challenge::new(target, current_time, requestor, domain, secret);

        let encoded = challenge.encode();
        let decoded = Challenge::decode(&encoded, secret).unwrap();

        assert_eq!(challenge, decoded);
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
