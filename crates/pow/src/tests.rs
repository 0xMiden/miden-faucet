use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{Challenge, ChallengeError, PoWRateLimiter, PoWRateLimiterConfig};

fn find_pow_solution(challenge: &Challenge, max_iterations: u64) -> Option<u64> {
    (0..max_iterations).find(|&nonce| challenge.validate_pow(nonce))
}

fn create_test_pow() -> PoWRateLimiter {
    let mut secret = [0u8; 32];
    secret[..12].copy_from_slice(b"miden-faucet");

    PoWRateLimiter::new(
        secret,
        PoWRateLimiterConfig {
            challenge_lifetime: Duration::from_secs(3),
            growth_rate: 1.0,
            cleanup_interval: Duration::from_millis(500),
            baseline: 0,
        },
    )
}

#[tokio::test]
async fn challenge_nonce_is_validated() {
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
        &challenge,
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
        &challenge,
        nonce,
        current_time,
        request_complexity,
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn challenge_timestamp_is_validated() {
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
        &challenge,
        nonce,
        current_time + pow.config.challenge_lifetime.as_secs() + 1,
        request_complexity,
    );
    assert!(result.is_err());

    // Submit challenge with correct timestamp - should succeed
    let result = pow.submit_challenge(
        requestor,
        domain,
        &challenge,
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

    let time_1 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let result =
        pow.submit_challenge(requestor, domain, &challenge, nonce, time_1, request_complexity);
    assert!(result.is_ok());

    // Try to submit second challenge - should fail because of rate limiting
    let challenge = pow.build_challenge(requestor, domain, request_complexity);
    let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");

    let time_2 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let result =
        pow.submit_challenge(requestor, domain, &challenge, nonce, time_2, request_complexity);
    assert!(result.is_err());
    let remaining_time = time_1 + pow.config.challenge_lifetime.as_secs() - time_2;
    let Err(ChallengeError::RateLimited(timestamp)) = result else {
        panic!("Expected RateLimited error");
    };
    assert_eq!(timestamp, remaining_time);

    // Try to submit it using a different api key - should succeed
    let domain = [2u8; 32];
    let challenge = pow.build_challenge(requestor, domain, request_complexity);
    let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");
    let result =
        pow.submit_challenge(requestor, domain, &challenge, nonce, time_2, request_complexity);
    assert!(result.is_ok());
}

#[tokio::test]
async fn requestor_is_rate_limited_after_challenge_expires() {
    let pow = create_test_pow();
    let domain = [1u8; 32];
    let requestor = [0u8; 32];
    let request_complexity = 1;

    // Request and solve challenge 1
    let challenge_1 = pow.build_challenge(requestor, domain, request_complexity);
    let nonce_1 = find_pow_solution(&challenge_1, 10000).expect("Should find solution");

    // Wait 1 second and request and solve challenge 2
    tokio::time::sleep(Duration::from_secs(1)).await;
    let challenge_2 = pow.build_challenge(requestor, domain, request_complexity);
    let nonce_2 = find_pow_solution(&challenge_2, 10000).expect("Should find solution");

    // Wait until challenge 1 is almost expired and submit it
    tokio::time::sleep(pow.config.challenge_lifetime - Duration::from_millis(1100)).await;
    let time_1 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let result =
        pow.submit_challenge(requestor, domain, &challenge_1, nonce_1, time_1, request_complexity);
    assert!(result.is_ok());

    // Wait 1 second until challenge 1 is expired and then submit challenge 2 should fail
    tokio::time::sleep(Duration::from_secs(1)).await;
    let time_2 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let result =
        pow.submit_challenge(requestor, domain, &challenge_2, nonce_2, time_2, request_complexity);
    assert!(result.is_err());
    let Err(ChallengeError::RateLimited(timestamp)) = result else {
        panic!("Expected RateLimited error");
    };
    assert_eq!(timestamp, pow.config.challenge_lifetime.as_secs() - 1);
}

#[tokio::test]
async fn difficuty_increases_with_submitted_challenges() {
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

    pow.submit_challenge(requestor, domain, &challenge, nonce, current_time, request_complexity)
        .unwrap();

    assert_eq!(pow.challenges.read().unwrap().num_challenges_for_domain(&domain), 1);
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
async fn submit_challenge_while_previous_one_is_not_cleaned_up() {
    let mut secret = [0u8; 32];
    secret[..12].copy_from_slice(b"miden-faucet");

    // setup pow with short challenge lifetime and long cleanup interval to test the case
    // where cleanup has not run yet but the challenge is expired.
    let pow = PoWRateLimiter::new(
        secret,
        PoWRateLimiterConfig {
            challenge_lifetime: Duration::from_secs(1),
            growth_rate: 1.0,
            cleanup_interval: Duration::from_secs(3),
            baseline: 0,
        },
    );
    let domain_1 = [1u8; 32];
    let requestor = [0u8; 32];
    let request_complexity = 1;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // submit first challenge
    let challenge = pow.build_challenge(requestor, domain_1, request_complexity);
    let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");
    let result = pow.submit_challenge(
        requestor,
        domain_1,
        &challenge,
        nonce,
        current_time,
        request_complexity,
    );
    assert!(result.is_ok());

    // submit another challenge with same timestamp but different domain
    let domain_2 = [2u8; 32];
    let challenge = pow.build_challenge(requestor, domain_2, request_complexity);
    let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");
    let result = pow.submit_challenge(
        requestor,
        domain_2,
        &challenge,
        nonce,
        current_time,
        request_complexity,
    );
    assert!(result.is_ok());

    // submit challenge that overrides the first one (same solver but previous challenge is
    // expired)
    tokio::time::sleep(pow.config.challenge_lifetime + Duration::from_secs(1)).await;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let challenge = pow.build_challenge(requestor, domain_1, request_complexity);
    let nonce = find_pow_solution(&challenge, 10000).expect("Should find solution");
    let result = pow.submit_challenge(
        requestor,
        domain_1,
        &challenge,
        nonce,
        current_time,
        request_complexity,
    );
    dbg!(&result);
    assert!(result.is_ok());

    // check that the first challenge is removed from the cache
    assert_eq!(pow.challenges.read().unwrap().num_challenges_for_domain(&domain_1), 1);
    assert_eq!(pow.challenges.read().unwrap().num_challenges_for_domain(&domain_2), 1);
}
