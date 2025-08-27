# Miden Faucet PoW

This crate provides the proof-of-work (PoW) functionality for the Miden Faucet, implementing a rate-limiting mechanism based on computational challenges.

The difficulty of these challenges automatically adjusts based on the number of active challenges, providing dynamic rate limiting.

## Components

### `PoW` Struct

The main struct that handles challenge generation, validation, and submission:

```rust
pub struct PoW {
    secret: [u8; 32],
    challenge_cache: Arc<Mutex<ChallengeCache>>,
    config: PoWConfig,
}
```

### `Challenge` Struct

Represents a cryptographic challenge that users must solve:

```rust
pub struct Challenge {
    pub target: u64,           // Difficulty target (lower = harder)
    pub timestamp: u64,        // Creation timestamp
    pub account_id: AccountId, // Associated account
    pub api_key: ApiKey,       // Associated API key
    pub signature: [u8; 32],   // Server signature
}
```

### Challenge Validation

A challenge solution is valid when:
1. The hash `H(challenge_string, nonce)` interpreted as a big-endian u64 is less than the target
2. The challenge hasn't expired
3. The challenge hasn't been used before
4. The account isn't rate-limited, i.e. it has not previously submitted a challenge that is still valid

## Dynamic Difficulty

The system automatically adjusts challenge difficulty based on usage:
- **Target calculation**: `max_target / difficulty`
- **Max target**: `u64::MAX >> baseline`
- **Difficulty**: `max(num_active_challenges << growth_rate, 1)`

This means as more users solve challenges, the difficulty increases exponentially, providing automatic rate limiting. Each API key has it's own difficulty based on it's usage.

## License

This project is [MIT licensed](../../LICENSE).
