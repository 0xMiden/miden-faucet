# Miden PoW Rate Limiter

This crate provides a proof-of-work (PoW) functionality, implementing a rate-limiting mechanism based on computational challenges.

The PoW rate limiter operates through a challenge-response mechanism:

1. **Challenge Generation**: When a requestor needs to perform an action, they request a challenge for a specific domain.
2. **Computational Work**: The requestor must find a nonce that, when hashed with the challenge, produces a result below a dynamically calculated target.
3. **Challenge Submission**: The solved challenge is submitted back to the rate limiter for validation.
4. **Rate Limiting**: Once a challenge is successfully submitted, the requestor is temporarily rate-limited.

## Domain-Based Rate Limiting

The PoW rate limiter uses a **domain** concept to provide isolated rate limiting for different services or use cases. A domain is a 32-byte identifier that represents a unique service or context that requests challenges.

- **Separate Difficulties**: Each domain maintains its own difficulty calculation and active challenge count.
- **Independent Rate Limiting**: Rate limiting is applied per domain, so different services don't interfere with each other.

### Configuration Settings

The `PoWRateLimiterConfig` struct allows you to customize the behavior of the rate limiter:

- **`challenge_lifetime`**: How long a challenge remains valid after generation. After this duration, challenges expire and cannot be submitted. It also sets for how long challenges will be stored and thus the requestor will be rate-limited. Choose based on expected solving time and security requirements.

- **`growth_rate`**: Controls how aggressively difficulty increases with more active challenges. The number of active challenges gets multiplied by the growth rate to compute the load difficulty, so higher values mean more aggressive rate limiting.

- **`baseline`**: Sets the initial difficulty baseline. Higher baseline values make challenges harder from the start. Range: 0-32.

- **`cleanup_interval`**: How often the system removes expired challenges from memory.

## Challenge Validation

A challenge solution is valid when:
1. The hash `H(challenge_string, nonce)` interpreted as a big-endian u64 is less than the target
2. The challenge hasn't expired
3. The requestor isn't rate-limited, i.e. it has not submitted a challenge in the last `challenge_lifetime` seconds.

## Dynamic Difficulty

The system automatically adjusts challenges difficulty based on usage and the request complexity. 

The challenge target $t$ is computed as follows: 

$$
t = \dfrac{2^{64}} {d}
$$

We call $d$ "difficulty" such that $log_2(d)$ is the number of bits of work that we need to do to solve the challenge. We compute it as: 

$$
d = 2^b \centerdot c \centerdot n \centerdot g
$$

Where:
- $b$ is the baseline difficulty (in bits).
- $c$ is the request complexity such that $log_2(c)$ is the request difficulty in bits.
- $n$ is the number of active requests.
- $g$ is the growth rate, represented as a floating-point value.

Overall, as more users solve challenges the difficulty increases, providing automatic rate limiting.

## License

This project is [MIT licensed](../../LICENSE).
