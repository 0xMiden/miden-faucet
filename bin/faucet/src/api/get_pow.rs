use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use http::StatusCode;
use miden_client::account::{AccountId, AccountIdError};
use serde::Deserialize;

use crate::error_report::ErrorReport;
use crate::pow::api_key::ApiKey;
use crate::pow::{PoW, PowRequest};

// ENDPOINT
// ================================================================================================

pub async fn get_pow(
    State(pow): State<PoW>,
    Query(params): Query<RawPowRequest>,
) -> Result<impl IntoResponse, PowRequestError> {
    let request = params.validate()?;
    let challenge = pow.build_challenge(request);
    Ok(Json(challenge))
}

// REQUEST VALIDATION
// ================================================================================================

/// Used to receive the initial `get_pow` request from the user.
#[derive(Deserialize)]
pub struct RawPowRequest {
    pub account_id: String,
    pub api_key: Option<String>,
}

impl RawPowRequest {
    pub fn validate(self) -> Result<PowRequest, PowRequestError> {
        let account_id = if self.account_id.starts_with("0x") {
            AccountId::from_hex(&self.account_id)
        } else {
            AccountId::from_bech32(&self.account_id).map(|(_, account_id)| account_id)
        }
        .map_err(PowRequestError::InvalidAccount)?;

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
    #[error("account address failed to parse")]
    InvalidAccount(#[source] AccountIdError),
    #[error("API key failed to parse")]
    InvalidApiKey(String),
}

impl PowRequestError {
    /// Take care to not expose internal errors here.
    fn user_facing_error(&self) -> String {
        match self {
            Self::InvalidAccount(error) => error.as_report(),
            Self::InvalidApiKey(_) => "Invalid API key".to_owned(),
        }
    }
}

impl IntoResponse for PowRequestError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, self.user_facing_error()).into_response()
    }
}
