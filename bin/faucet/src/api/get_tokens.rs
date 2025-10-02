use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use miden_client::account::{AccountId, Address};
use miden_faucet_lib::requests::{MintError, MintRequest, MintRequestSender};
use miden_faucet_lib::types::{AssetAmount, AssetAmountError, NoteType};
use miden_pow_rate_limiter::ChallengeError;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::oneshot;
use tracing::instrument;

use crate::COMPONENT;
use crate::api::{AccountError, Server};
use crate::api_key::ApiKey;
use crate::error_report::ErrorReport;

// ENDPOINT
// ================================================================================================

#[instrument(
    parent = None, target = COMPONENT, name = "faucet.server.get_tokens", skip_all,
    fields(
        account_id = %request.account_id,
        is_private_note = %request.is_private_note,
        asset_amount = %request.asset_amount,
    )
)]
pub async fn get_tokens(
    State(server): State<Server>,
    Query(request): Query<RawMintRequest>,
) -> Result<Json<GetTokensResponse>, GetTokenError> {
    let (mint_response_sender, mint_response_receiver) = oneshot::channel();

    let validated_request = request.validate(&server).map_err(GetTokenError::InvalidRequest)?;
    let requested_amount = validated_request.asset_amount.base_units();

    let span = tracing::Span::current();
    span.record("account", validated_request.account_id.to_hex());
    span.record("amount", requested_amount);
    span.record("note_type", validated_request.note_type.to_string());

    server
        .mint_state
        .request_sender
        .try_send((validated_request, mint_response_sender))
        .map_err(|err| match err {
            TrySendError::Full(_) => GetTokenError::FaucetOverloaded,
            TrySendError::Closed(_) => GetTokenError::FaucetClosed,
        })?;

    let mint_response = mint_response_receiver
        .await
        .map_err(|_| GetTokenError::FaucetReturnChannelClosed)?
        .map_err(GetTokenError::MintError)?;

    Ok(Json(GetTokensResponse {
        tx_id: mint_response.tx_id.to_string(),
        note_id: mint_response.note_id.to_string(),
    }))
}

#[derive(Serialize)]
pub struct GetTokensResponse {
    tx_id: String,
    note_id: String,
}

// STATE
// ================================================================================================

#[derive(Clone)]
pub struct GetTokensState {
    pub request_sender: MintRequestSender,
    pub max_claimable_amount: AssetAmount,
}

impl GetTokensState {
    pub fn new(request_sender: MintRequestSender, max_claimable_amount: AssetAmount) -> Self {
        Self { request_sender, max_claimable_amount }
    }
}

// REQUEST VALIDATION
// ================================================================================================

/// Used to receive the initial request from the user.
///
/// Further parsing is done to get the expected [`MintRequest`] expected by the faucet client.
#[derive(Deserialize)]
pub struct RawMintRequest {
    pub account_id: String,
    pub is_private_note: bool,
    pub asset_amount: u64,
    pub challenge: Option<String>,
    pub nonce: Option<u64>,
    pub api_key: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum MintRequestError {
    #[error("account error")]
    AccountError(#[source] AccountError),
    #[error("requested amount {0} exceeds the maximum claimable amount of {1}")]
    AssetAmountTooBig(AssetAmount, AssetAmount),
    #[error("requested amount {0} is not a valid asset amount")]
    InvalidAssetAmount(AssetAmountError),
    #[error("PoW error")]
    PowError(#[from] ChallengeError),
    #[error("API key {0} is invalid")]
    InvalidApiKey(String),
    #[error("PoW parameters are missing")]
    MissingPowParameters,
}

pub enum GetTokenError {
    InvalidRequest(MintRequestError),
    MintError(MintError),
    FaucetOverloaded,
    FaucetClosed,
    FaucetReturnChannelClosed,
}

impl GetTokenError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidRequest(MintRequestError::PowError(ChallengeError::RateLimited(_))) => {
                StatusCode::TOO_MANY_REQUESTS
            },
            Self::InvalidRequest(_) | Self::MintError(_) => StatusCode::BAD_REQUEST,
            Self::FaucetOverloaded | Self::FaucetClosed => StatusCode::SERVICE_UNAVAILABLE,
            Self::FaucetReturnChannelClosed => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Take care to not expose internal errors here.
    fn user_facing_error(&self) -> String {
        match self {
            Self::InvalidRequest(error) => error.as_report(),
            Self::MintError(error) => error.as_report(),
            Self::FaucetOverloaded => {
                "The faucet is currently overloaded, please try again later.".to_owned()
            },
            Self::FaucetClosed => {
                "The faucet is currently unavailable, please try again later.".to_owned()
            },
            Self::FaucetReturnChannelClosed => "Internal error.".to_owned(),
        }
    }

    /// Write a trace log for the error, if applicable.
    fn trace(&self) {
        match self {
            Self::InvalidRequest(_) | Self::MintError(_) => {},
            Self::FaucetOverloaded => tracing::warn!("faucet client is overloaded"),
            Self::FaucetClosed => {
                tracing::error!("faucet channel is closed but requests are still coming in");
            },
            Self::FaucetReturnChannelClosed => {
                tracing::error!("result channel from the faucet closed mid-request");
            },
        }
    }

    /// Returns headers for the error response. In case of a rate limited error, the Retry-After
    /// header is set. Otherwise, just returns an empty header map.
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Self::InvalidRequest(MintRequestError::PowError(ChallengeError::RateLimited(
            timestamp,
        ))) = self
        {
            headers.insert(axum::http::header::RETRY_AFTER, HeaderValue::from(*timestamp));
        }
        headers
    }
}

impl IntoResponse for GetTokenError {
    fn into_response(self) -> Response {
        self.trace();
        (self.headers(), (self.status_code(), self.user_facing_error())).into_response()
    }
}

impl RawMintRequest {
    /// Further validates a raw request, turning it into a valid [`MintRequest`] which can be
    /// submitted to the faucet client.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///   - the account ID is not a valid hex string
    ///   - the asset amount is not one of the provided options
    ///   - the API key is invalid
    ///   - the challenge is missing or invalid
    ///   - the nonce is missing or doesn't solve the challenge
    ///   - the challenge timestamp is expired
    ///   - the challenge has already been used
    #[instrument(level = "debug", target = COMPONENT, name = "faucet.server.validate", skip_all)]
    fn validate(self, server: &Server) -> Result<MintRequest, MintRequestError> {
        let note_type = if self.is_private_note {
            NoteType::Private
        } else {
            NoteType::Public
        };

        let account_id = if self.account_id.starts_with("0x") {
            AccountId::from_hex(&self.account_id).map_err(AccountError::ParseId)
        } else {
            Address::from_bech32(&self.account_id)
                .map_err(AccountError::ParseAddress)
                .and_then(|(_, address)| match address {
                    Address::AccountId(account_id_address) => Ok(account_id_address.id()),
                    _ => Err(AccountError::AddressNotIdBased),
                })
        }
        .map_err(MintRequestError::AccountError)?;

        let asset_amount =
            AssetAmount::new(self.asset_amount).map_err(MintRequestError::InvalidAssetAmount)?;
        if asset_amount > server.mint_state.max_claimable_amount {
            return Err(MintRequestError::AssetAmountTooBig(
                asset_amount,
                server.mint_state.max_claimable_amount,
            ));
        }

        // Check the API key, if provided
        let api_key = self.api_key.as_deref().map(ApiKey::decode).transpose()?;
        if let Some(api_key) = &api_key
            && !server.api_keys.contains(api_key)
        {
            return Err(MintRequestError::InvalidApiKey(api_key.encode()));
        }

        // Validate Challenge and nonce
        let challenge_str = self.challenge.ok_or(MintRequestError::MissingPowParameters)?;
        let nonce = self.nonce.ok_or(MintRequestError::MissingPowParameters)?;
        let request_complexity = (asset_amount.base_units() / server.metadata.base_amount) + 1;

        server.submit_challenge(
            &challenge_str,
            nonce,
            account_id,
            api_key.unwrap_or_default(),
            request_complexity,
        )?;

        Ok(MintRequest { account_id, note_type, asset_amount })
    }
}
