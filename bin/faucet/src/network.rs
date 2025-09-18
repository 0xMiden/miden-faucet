use std::convert::Infallible;
use std::str::FromStr;

use miden_client::account::{NetworkId, NetworkIdError};
use miden_client::rpc::Endpoint;
use serde::{Deserialize, Serialize};

// NETWORK
// ================================================================================================

/// Represents the network where the faucet is running. It is used to display the correct bech32
/// addresses in the UI.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum FaucetNetwork {
    Testnet,
    Devnet,
    Localhost,
    Custom(String),
}

impl FromStr for FaucetNetwork {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Infallible> {
        match s.to_lowercase().as_str() {
            "devnet" => Ok(FaucetNetwork::Devnet),
            "localhost" => Ok(FaucetNetwork::Localhost),
            "testnet" => Ok(FaucetNetwork::Testnet),
            custom => Ok(FaucetNetwork::Custom(custom.to_string())),
        }
    }
}

impl FaucetNetwork {
    /// Converts the network configuration to a network ID.
    pub fn to_network_id(&self) -> Result<NetworkId, NetworkIdError> {
        Ok(match self {
            FaucetNetwork::Testnet => NetworkId::Testnet,
            FaucetNetwork::Devnet => NetworkId::Devnet,
            FaucetNetwork::Localhost => NetworkId::new("mlcl")?,
            FaucetNetwork::Custom(s) => NetworkId::new(s)?,
        })
    }

    /// Converts the Network variant to its corresponding RPC endpoint string, if it exists.
    pub fn to_rpc_endpoint(&self) -> Option<String> {
        match self {
            FaucetNetwork::Custom(_) => None,
            FaucetNetwork::Devnet => Some(Endpoint::devnet().to_string()),
            FaucetNetwork::Localhost => Some(Endpoint::localhost().to_string()),
            FaucetNetwork::Testnet => Some(Endpoint::testnet().to_string()),
        }
    }
}
