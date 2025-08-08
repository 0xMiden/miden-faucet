use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use axum::Json;
use axum::extract::State;
use miden_faucet_client::FaucetId;
use miden_faucet_client::types::AssetOptions;

/// Describes the faucet metadata.
///
/// More specifically, the faucet's account ID and allowed mint amounts.
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
