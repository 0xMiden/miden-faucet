use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use miden_client::utils::RwLock;
use miden_faucet_lib::FaucetId;
use miden_faucet_lib::types::AssetAmount;
use serde::{Serialize, Serializer};
use tracing::instrument;
use url::Url;

use crate::COMPONENT;

/// Describes the faucet metadata needed to show on the frontend.
pub struct Metadata {
    pub id: FaucetId,
    pub issuance: Arc<RwLock<AssetAmount>>,
    pub max_supply: AssetAmount,
    pub decimals: u8,
    pub explorer_url: Option<Url>,
}

impl Serialize for Metadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Metadata", 5)?;
        state.serialize_field("id", &self.id.to_bech32())?;
        state.serialize_field("issuance", &self.issuance.read().base_units())?;
        state.serialize_field("max_supply", &self.max_supply.base_units())?;
        state.serialize_field("decimals", &self.decimals)?;
        state.serialize_field("explorer_url", &self.explorer_url)?;
        state.end()
    }
}

// ENDPOINT
// ================================================================================================

#[instrument(parent = None, target = COMPONENT, name = "faucet.server.get_metadata", skip_all)]
pub async fn get_metadata(State(metadata): State<&'static Metadata>) -> Json<&'static Metadata> {
    Json(metadata)
}
