//! CLI command to mint tokens from a remote faucet

use std::time::Duration;

use clap::Parser;
use miden_client::account::{AccountId, Address};
use miden_client::address::AddressId;
use miden_client::note::NoteId;
use rand::Rng;
use reqwest::{Client as HttpClient, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::task;

// CONSTANTS
// =================================================================================================

const DEFAULT_FAUCET_URL: &str = "https://faucet-api.testnet.miden.io";
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

// CLI
// =================================================================================================

/// Mint tokens from a remote faucet by solving its `PoW` challenge and requesting a **public**
/// P2ID note.
#[derive(Debug, Parser, Clone)]
pub struct MintCmd {
    /// Faucet API base URL. Defaults to the public testnet faucet.
    #[arg(long = "url", default_value = DEFAULT_FAUCET_URL, value_name = "URL")]
    api_url: String,

    /// Account ID or address to receive the minted tokens.
    #[arg(short = 'a', long = "account", value_name = "ACCOUNT")]
    account: String,

    /// Amount to mint (in base units).
    #[arg(short = 'm', long = "amount", value_name = "U64")]
    amount: u64,

    /// Optional faucet API key.
    #[arg(long = "api-key", value_name = "STRING")]
    api_key: Option<String>,
}

impl MintCmd {
    /// Executes the mint command.
    pub async fn execute(&self) -> Result<(), MintClientError> {
        if self.amount == 0 {
            return Err(MintClientError::AmountZero);
        }

        let account_id = parse_account_id(&self.account)?;
        let faucet_client =
            FaucetHttpClient::new(&self.api_url, DEFAULT_TIMEOUT_MS, self.api_key.clone())?;

        println!(
            "Requesting PoW challenge for account {} from faucet at {}...",
            account_id.to_hex(),
            faucet_client.base_url
        );

        let (challenge, target) = faucet_client.request_pow(&account_id, self.amount).await?;

        println!("Solving faucet PoW challenge, this can take some time...");
        let nonce = solve_challenge(&challenge, target).await?;

        println!("Submitting mint request for a public P2ID note...");
        let minted_note = faucet_client
            .request_tokens(&challenge, nonce, &account_id, self.amount)
            .await?;

        println!("Mint request accepted. Transaction: {}", minted_note.tx_id);
        println!("Public P2ID note commitment: {}", minted_note.note_id.to_hex());

        Ok(())
    }
}

// HTTP CLIENT
// =================================================================================================

/// HTTP client for interacting with the faucet API.
#[derive(Clone)]
struct FaucetHttpClient {
    http_client: HttpClient,
    base_url: Url,
    api_key: Option<String>,
}

impl FaucetHttpClient {
    /// Creates a new `FaucetHttpClient` instance.
    fn new(
        endpoint: &str,
        timeout_ms: u64,
        api_key: Option<String>,
    ) -> Result<Self, MintClientError> {
        let base_url = Url::parse(endpoint)
            .map_err(|err| MintClientError::InvalidUrl(endpoint.to_owned(), err))?;

        let http_client = HttpClient::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .map_err(MintClientError::HttpClient)?;

        Ok(Self { http_client, base_url, api_key })
    }

    /// Requests a `PoW` challenge from the faucet API.
    async fn request_pow(
        &self,
        account_id: &AccountId,
        amount: u64,
    ) -> Result<(String, u64), MintClientError> {
        let pow_url = self
            .base_url
            .join("pow")
            .map_err(|err| MintClientError::InvalidUrl(self.base_url.to_string(), err))?;

        let mut request = self
            .http_client
            .get(pow_url)
            .query(&[("account_id", account_id.to_hex()), ("amount", amount.to_string())]);

        if let Some(key) = &self.api_key {
            request = request.query(&[("api_key", key)]);
        }

        let response = request.send().await.map_err(|err| MintClientError::Request("PoW", err))?;

        if !response.status().is_success() {
            let status = response.status();
            let body =
                response.text().await.map_err(|err| MintClientError::ResponseBody("pow", err))?;
            return Err(MintClientError::UnexpectedStatus("pow", status, body));
        }

        let body =
            response.text().await.map_err(|err| MintClientError::ResponseBody("pow", err))?;
        let parsed = serde_json::from_str::<PowResponse>(&body)
            .map_err(|err| MintClientError::ParseResponse("PoW", err, body.clone()))?;

        Ok((parsed.challenge, parsed.target))
    }

    /// Requests tokens from the faucet API.
    async fn request_tokens(
        &self,
        challenge: &str,
        nonce: u64,
        account_id: &AccountId,
        amount: u64,
    ) -> Result<MintNote, MintClientError> {
        let url = self
            .base_url
            .join("get_tokens")
            .map_err(|err| MintClientError::InvalidUrl(self.base_url.to_string(), err))?;

        let mut request = self.http_client.get(url).query(&[
            ("account_id", account_id.to_hex()),
            ("asset_amount", amount.to_string()),
            ("is_private_note", false.to_string()),
            ("challenge", challenge.to_owned()),
            ("nonce", nonce.to_string()),
        ]);

        if let Some(key) = &self.api_key {
            request = request.query(&[("api_key", key)]);
        }

        let response = request
            .send()
            .await
            .map_err(|err| MintClientError::Request("get_tokens", err))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|err| MintClientError::ResponseBody("get_tokens", err))?;
            return Err(MintClientError::UnexpectedStatus("get_tokens", status, body));
        }

        let body = response
            .text()
            .await
            .map_err(|err| MintClientError::ResponseBody("get_tokens", err))?;
        let parsed = serde_json::from_str::<GetTokensResponse>(&body)
            .map_err(|err| MintClientError::ParseResponse("get_tokens", err, body.clone()))?;

        let note_id = NoteId::try_from_hex(&parsed.note_id).map_err(|err| {
            MintClientError::InvalidNoteId(parsed.note_id.clone(), err.to_string())
        })?;

        Ok(MintNote { note_id, tx_id: parsed.tx_id })
    }
}

// RESPONSES
// =================================================================================================

/// Response from the `/pow` endpoint.
#[derive(Debug, Deserialize, Serialize, Clone)]
struct PowResponse {
    challenge: String,
    target: u64,
}

/// Response from the `/get_tokens` endpoint.
#[derive(Debug, Deserialize, Serialize, Clone)]
struct GetTokensResponse {
    note_id: String,
    tx_id: String,
}

/// Represents a minted note with its ID and transaction ID.
#[derive(Debug, Clone)]
struct MintNote {
    note_id: NoteId,
    tx_id: String,
}

// ERRORS
// =================================================================================================

/// Errors that can occur while interacting with the faucet API.
#[derive(Debug, thiserror::Error)]
pub enum MintClientError {
    #[error("amount must be greater than zero")]
    AmountZero,
    #[error("invalid account `{0}`: {1}")]
    InvalidAccount(String, String),
    #[error("invalid faucet URL `{0}`: {1}")]
    InvalidUrl(String, url::ParseError),
    #[error("failed to build HTTP client: {0}")]
    HttpClient(#[source] reqwest::Error),
    #[error("{0} request failed: {1}")]
    Request(&'static str, #[source] reqwest::Error),
    #[error("{0} request failed with status {1}: {2}")]
    UnexpectedStatus(&'static str, reqwest::StatusCode, String),
    #[error("failed to parse {0} response: {1}. Body: {2}")]
    ParseResponse(&'static str, #[source] serde_json::Error, String),
    #[error("failed to read {0} response body: {1}")]
    ResponseBody(&'static str, #[source] reqwest::Error),
    #[error("faucet returned a PoW target of 0")]
    ZeroTarget,
    #[error("invalid challenge bytes returned by faucet: {0}")]
    InvalidChallenge(#[source] hex::FromHexError),
    #[error("PoW solving task failed: {0}")]
    PowTask(String),
    #[error("invalid note id `{0}`: {1}")]
    InvalidNoteId(String, String),
}

// HELPERS
// =================================================================================================

/// Parses a user provided account ID string and returns the corresponding `AccountId`
fn parse_account_id(input: &str) -> Result<AccountId, MintClientError> {
    if input.starts_with("0x") {
        AccountId::from_hex(input)
            .map_err(|err| MintClientError::InvalidAccount(input.to_owned(), err.to_string()))
    } else {
        Address::decode(input)
            .map_err(|err| MintClientError::InvalidAccount(input.to_owned(), err.to_string()))
            .and_then(|(_, address)| match address.id() {
                AddressId::AccountId(account_id) => Ok(account_id),
                _ => Err(MintClientError::InvalidAccount(
                    input.to_owned(),
                    "address is not account-based".to_owned(),
                )),
            })
    }
}

/// Solves the `PoW` challenge and returns the nonce that satisfies the target.
///
/// The faucet expects the first 8 bytes of the SHA-256 digest (big endian) to be lower than
/// the target.
///
/// Heavy work runs on a blocking thread so we don't stall the async runtime
async fn solve_challenge(challenge_hex: &str, target: u64) -> Result<u64, MintClientError> {
    if target == 0 {
        return Err(MintClientError::ZeroTarget);
    }

    let challenge_bytes = hex::decode(challenge_hex).map_err(MintClientError::InvalidChallenge)?;

    task::spawn_blocking(move || -> Result<u64, MintClientError> {
        let mut rng = rand::rng();

        loop {
            let nonce: u64 = rng.random();

            let mut hasher = Sha256::new();
            hasher.update(&challenge_bytes);
            hasher.update(nonce.to_be_bytes());
            let hash = hasher.finalize();
            let digest =
                u64::from_be_bytes(hash[..8].try_into().expect("hash should be 32 bytes long"));

            if digest < target {
                return Ok(nonce);
            }
        }
    })
    .await
    .map_err(|err| MintClientError::PowTask(err.to_string()))?
}

// TESTS
// =================================================================================================

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::extract::{Query, State};
    use axum::routing::get;
    use axum::{Json, Router};
    use miden_client::account::AccountId;
    use miden_client::note::NoteId;
    use serde::Deserialize;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    use super::{GetTokensResponse, MintCmd, MintNote, PowResponse};

    #[derive(Clone, Default)]
    struct RecordedRequest {
        account_id: Option<String>,
        amount: Option<u64>,
        is_private_note: Option<String>,
        api_key: Option<String>,
        challenge: Option<String>,
    }

    #[derive(Clone)]
    struct AppState {
        pow_response: PowResponse,
        note: MintNote,
        recorded: Arc<Mutex<RecordedRequest>>,
    }

    #[derive(Deserialize)]
    struct PowQuery {
        amount: u64,
        account_id: String,
        api_key: Option<String>,
    }

    #[derive(Deserialize)]
    struct TokensQuery {
        account_id: String,
        is_private_note: String,
        asset_amount: u64,
        challenge: String,
        #[allow(dead_code)]
        nonce: u64,
        api_key: Option<String>,
    }

    #[tokio::test]
    async fn mint_command_requests_public_note() {
        let account_hex = "0xca8203e8e58cf72049b061afca78ce";
        let account_id = AccountId::from_hex(account_hex).unwrap();
        let expected_amount = 123_000;
        let pow_response = PowResponse {
            challenge: "00".repeat(32),
            target: u64::MAX,
        };
        let note_id_hex = format!("0x{}", "00".repeat(32));
        let note_id =
            NoteId::try_from_hex(&note_id_hex).expect("hex string should produce a note id");
        let app_state = AppState {
            pow_response,
            note: MintNote { note_id, tx_id: "0xdeadbeef".to_string() },
            recorded: Arc::new(Mutex::new(RecordedRequest::default())),
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/pow", get(pow_handler))
            .route("/get_tokens", get(tokens_handler))
            .with_state(app_state.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let cli = MintCmd {
            api_url: format!("http://{addr}"),
            account: account_id.to_hex(),
            amount: expected_amount,
            api_key: Some("test-key".to_owned()),
        };

        cli.execute().await.unwrap();

        let recorded = app_state.recorded.lock().await.clone();
        assert_eq!(recorded.account_id, Some(account_id.to_hex()));
        assert_eq!(recorded.amount, Some(expected_amount));
        assert_eq!(recorded.is_private_note.as_deref(), Some("false"));
        assert_eq!(recorded.api_key.as_deref(), Some("test-key"));
        assert_eq!(recorded.challenge, Some("00".repeat(32)));
    }

    async fn pow_handler(
        State(state): State<AppState>,
        Query(params): Query<PowQuery>,
    ) -> Json<PowResponse> {
        {
            let mut recorded = state.recorded.lock().await;
            recorded.account_id = Some(params.account_id);
            recorded.amount = Some(params.amount);
            recorded.api_key = params.api_key;
        }
        Json(state.pow_response.clone())
    }

    async fn tokens_handler(
        State(state): State<AppState>,
        Query(params): Query<TokensQuery>,
    ) -> Json<GetTokensResponse> {
        {
            let mut recorded = state.recorded.lock().await;
            recorded.account_id = Some(params.account_id.clone());
            recorded.amount = Some(params.asset_amount);
            recorded.is_private_note = Some(params.is_private_note.clone());
            recorded.api_key = params.api_key.clone();
            recorded.challenge = Some(params.challenge);
        }
        Json(GetTokensResponse {
            note_id: state.note.note_id.to_hex(),
            tx_id: state.note.tx_id.clone(),
        })
    }
}
