mod faucet;
mod server;
mod types;

pub use faucet::Faucet;
pub use server::{ApiKey, PoWConfig, Server};

// CONSTANTS
// =================================================================================================

const COMPONENT: &str = "miden-faucet-backend";
