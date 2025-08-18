use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use axum::Json;
use axum::extract::State;
use miden_faucet_lib::FaucetId;
use miden_faucet_lib::types::AssetOptions;

/// Describes the faucet metadata needed to show on the frontend.
#[derive(Clone, serde::Serialize)]
pub struct Metadata {
    pub id: FaucetId,
    pub asset_amount_options: AssetOptions,
    pub issuance: Arc<AtomicU64>,
    pub max_supply: u64,
}

// ENDPOINT
// ================================================================================================

pub async fn get_metadata(State(metadata): State<&'static Metadata>) -> Json<&'static Metadata> {
    Json(metadata)
}
