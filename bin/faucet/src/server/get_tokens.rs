use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use miden_client::{
    account::{AccountId, AccountIdError},
    note::NoteId,
    transaction::TransactionId,
};
use miden_node_utils::ErrorReport;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc::error::TrySendError, oneshot};
use tracing::{error, instrument};

use super::Server;
use crate::{
    COMPONENT,
    network::ExplorerUrl,
    server::{ApiKey, MintRequestSender},
    types::{AssetAmount, AssetOptions, NoteType},
};

// ENDPOINT
// ================================================================================================

pub type MintResponseSender = oneshot::Sender<Result<MintResponse, MintRequestError>>;

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
) -> Result<impl IntoResponse, GetTokenError> {
    let (mint_response_sender, mint_response_receiver) = oneshot::channel();

    let validated_request = request.validate(&server).map_err(GetTokenError::InvalidRequest)?;
    let requested_amount = validated_request.asset_amount.inner();

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
        .map_err(GetTokenError::InvalidRequest)?;

    server.increment_claimed_supply(requested_amount);
    Ok(Json(mint_response))
}

// STATE
// ================================================================================================

#[derive(Clone)]
pub struct GetTokensState {
    pub request_sender: MintRequestSender,
    pub asset_options: AssetOptions,
}

impl GetTokensState {
    pub fn new(request_sender: MintRequestSender, asset_options: AssetOptions) -> Self {
        Self { request_sender, asset_options }
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
    #[error("account ID failed to parse")]
    AccountId(#[source] AccountIdError),
    #[error("asset amount {0} is not one of the provided options")]
    AssetAmount(u64),
    #[error("API key {0} is invalid")]
    InvalidApiKey(String),
    #[error("invalid POW solution")]
    InvalidPoW,
    #[error("POW parameters are missing")]
    MissingPowParameters,
    #[error("server signatures do not match")]
    ServerSignaturesDoNotMatch,
    #[error("server timestamp expired, received: {0}, current time: {1}")]
    ExpiredServerTimestamp(u64, u64),
    #[error("challenge already used")]
    ChallengeAlreadyUsed,
    #[error("account is rate limited")]
    RateLimited,
    #[error("faucet supply exceeded")]
    AvailableSupplyExceeded,
}

pub enum GetTokenError {
    InvalidRequest(MintRequestError),
    FaucetOverloaded,
    FaucetClosed,
    FaucetReturnChannelClosed,
}

impl GetTokenError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            Self::FaucetOverloaded | Self::FaucetClosed => StatusCode::SERVICE_UNAVAILABLE,
            Self::FaucetReturnChannelClosed => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Take care to not expose internal errors here.
    fn user_facing_error(&self) -> String {
        match self {
            Self::InvalidRequest(invalid_request) => invalid_request.as_report(),
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
            Self::InvalidRequest(_) => {},
            Self::FaucetOverloaded => tracing::warn!("faucet client is overloaded"),
            Self::FaucetClosed => {
                tracing::error!("faucet channel is closed but requests are still coming in");
            },
            Self::FaucetReturnChannelClosed => {
                tracing::error!("result channel from the faucet closed mid-request");
            },
        }
    }
}

impl IntoResponse for GetTokenError {
    fn into_response(self) -> Response {
        self.trace();
        (self.status_code(), self.user_facing_error()).into_response()
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
            AccountId::from_hex(&self.account_id)
        } else {
            AccountId::from_bech32(&self.account_id).map(|(_, account_id)| account_id)
        }
        .map_err(MintRequestError::AccountId)?;

        let asset_amount = server
            .mint_state
            .asset_options
            .validate(self.asset_amount)
            .ok_or(MintRequestError::AssetAmount(self.asset_amount))?;

        // Check the API key, if provided
        let api_key = self.api_key.as_deref().map(ApiKey::decode).transpose()?;
        if let Some(api_key) = &api_key {
            if !server.api_keys.contains(api_key) {
                return Err(MintRequestError::InvalidApiKey(api_key.encode()));
            }
        }

        // Validate Challenge and nonce
        let challenge_str = self.challenge.ok_or(MintRequestError::MissingPowParameters)?;
        let nonce = self.nonce.ok_or(MintRequestError::MissingPowParameters)?;

        server.submit_challenge(&challenge_str, nonce, account_id, &api_key.unwrap_or_default())?;

        Ok(MintRequest { account_id, note_type, asset_amount })
    }
}

/// A request for minting to the Faucet.
pub struct MintRequest {
    /// Destination account.
    pub account_id: AccountId,
    /// Whether to generate a public or private note to hold the minted asset.
    pub note_type: NoteType,
    /// The amount to mint.
    pub asset_amount: AssetAmount,
}

pub struct MintResponse {
    pub tx_id: TransactionId,
    pub note_id: NoteId,
    pub explorer_url: Option<ExplorerUrl>,
}

impl Serialize for MintResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("MintResponse", 3)?;
        state.serialize_field("tx_id", &self.tx_id.to_string())?;
        state.serialize_field("note_id", &self.note_id.to_string())?;
        state.serialize_field("explorer_url", &self.explorer_url)?;
        state.end()
    }
}
