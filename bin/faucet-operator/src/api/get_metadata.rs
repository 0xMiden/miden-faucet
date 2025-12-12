use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use miden_client::utils::RwLock;
use miden_faucet_lib::FaucetId;
use miden_faucet_lib::types::AssetAmount;
use serde::Serialize;
use tracing::instrument;
use url::Url;

use crate::COMPONENT;
use crate::api::ApiServer;
use crate::api_key::ApiKey;

/// Describes the faucet metadata needed to show on the frontend.
#[derive(Clone)]
pub struct Metadata {
    pub id: FaucetId,
    pub issuance: Arc<RwLock<AssetAmount>>,
    pub max_supply: AssetAmount,
    pub decimals: u8,
    pub explorer_url: Option<Url>,
    pub base_amount: u64,
}

// ENDPOINT
// ================================================================================================

#[instrument(parent = None, target = COMPONENT, name = "server.get_metadata", skip_all)]
pub async fn get_metadata(State(server): State<ApiServer>) -> Json<GetMetadataResponse> {
    let metadata = server.metadata;
    let issuance = metadata.issuance.read().base_units();
    Json(GetMetadataResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        id: metadata.id.to_bech32(),
        issuance,
        max_supply: metadata.max_supply.base_units(),
        decimals: metadata.decimals,
        explorer_url: metadata.explorer_url,
        pow_load_difficulty: server.rate_limiter.get_load_difficulty(ApiKey::default()),
        base_amount: metadata.base_amount,
    })
}

#[derive(Serialize)]
pub struct GetMetadataResponse {
    pub version: String,
    pub id: String,
    pub issuance: u64,
    pub max_supply: u64,
    pub decimals: u8,
    pub explorer_url: Option<Url>,
    pub pow_load_difficulty: u64,
    pub base_amount: u64,
}
