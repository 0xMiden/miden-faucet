use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use http::StatusCode;
use miden_client::account::{AccountId, Address};
use miden_pow_rate_limiter::{ApiKey, Challenge, PoW};
use serde::Deserialize;

use crate::api::AccountError;
use crate::error_report::ErrorReport;

// ENDPOINT
// ================================================================================================

pub async fn get_pow(
    State(pow): State<PoW>,
    Query(params): Query<RawPowRequest>,
) -> Result<Json<Challenge>, PowRequestError> {
    let request = params.validate()?;
    let challenge = pow.build_challenge(request.account_id, request.api_key);
    Ok(Json(challenge))
}

// REQUEST VALIDATION
// ================================================================================================

/// Validated and parsed request for the `PoW` challenge.
pub struct PowRequest {
    pub account_id: AccountId,
    pub api_key: ApiKey,
}

/// Used to receive the initial `get_pow` request from the user.
#[derive(Deserialize)]
pub struct RawPowRequest {
    account_id: String,
    api_key: Option<String>,
}

impl RawPowRequest {
    pub fn validate(self) -> Result<PowRequest, PowRequestError> {
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
        .map_err(PowRequestError::AccountError)?;

        let api_key = self
            .api_key
            .as_deref()
            .map(ApiKey::decode)
            .transpose()
            .map_err(|_| PowRequestError::InvalidApiKey(self.api_key.unwrap_or_default()))?
            .unwrap_or_default();

        Ok(PowRequest { account_id, api_key })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PowRequestError {
    #[error("account error")]
    AccountError(#[source] AccountError),
    #[error("API key failed to parse")]
    InvalidApiKey(String),
}

impl PowRequestError {
    /// Take care to not expose internal errors here.
    fn user_facing_error(&self) -> String {
        match self {
            Self::AccountError(error) => error.as_report(),
            Self::InvalidApiKey(_) => "Invalid API key".to_owned(),
        }
    }
}

impl IntoResponse for PowRequestError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, self.user_facing_error()).into_response()
    }
}
