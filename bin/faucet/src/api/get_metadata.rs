use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use miden_client::utils::RwLock;
use miden_faucet_lib::FaucetId;
use miden_faucet_lib::types::{AssetAmount, AssetOptions};
use serde::{Serialize, Serializer};

/// Describes the faucet metadata needed to show on the frontend.
pub struct Metadata {
    pub id: FaucetId,
    pub asset_amount_options: AssetOptions,
    pub issuance: Arc<RwLock<AssetAmount>>,
    pub max_supply: AssetAmount,
    pub decimals: u8,
}

impl Serialize for Metadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Metadata", 5)?;
        state.serialize_field("id", &self.id.to_bech32())?;
        state.serialize_field("asset_amount_options", &self.asset_amount_options.base_units())?;
        state.serialize_field("issuance", &self.issuance.read().base_units())?;
        state.serialize_field("max_supply", &self.max_supply.base_units())?;
        state.serialize_field("decimals", &self.decimals)?;
        state.end()
    }
}

// ENDPOINT
// ================================================================================================

pub async fn get_metadata(State(metadata): State<&'static Metadata>) -> Json<&'static Metadata> {
    Json(metadata)
}
