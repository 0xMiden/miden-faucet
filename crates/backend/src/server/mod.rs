use std::{
    collections::HashSet,
    convert::Infallible,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{Router, extract::FromRef, response::sse::Event, routing::get};
use get_tokens::{GetTokensState, get_tokens};
use http::{HeaderValue, Request};
use miden_faucet_common::AssetOptions;
use miden_objects::account::AccountId;
use pow::PoW;
use sha3::{Digest, Sha3_256};
use tokio::sync::mpsc;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    set_header::SetResponseHeaderLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::Level;

use crate::{
    faucet::MintRequest,
    server::{get_pow::get_pow, get_tokens::MintRequestError},
};

mod api_key;
mod challenge;
mod get_pow;
mod get_tokens;
mod pow;

// RE-EXPORTS
// ================================================================================================

pub use api_key::ApiKey;
pub use pow::PoWConfig;

// FAUCET STATE
// ================================================================================================

type RequestSender = mpsc::Sender<(MintRequest, mpsc::Sender<Result<Event, Infallible>>)>;

/// Serves the faucet's backend and handles token requests.
#[derive(Clone)]
pub struct Server {
    mint_state: GetTokensState,
    pow: PoW,
    api_keys: HashSet<ApiKey>,
}

impl Server {
    pub fn new(
        asset_options: AssetOptions,
        request_sender: RequestSender,
        pow_secret: &str,
        pow_config: PoWConfig,
        api_keys: &[ApiKey],
    ) -> Self {
        let mint_state = GetTokensState::new(request_sender, asset_options);

        // Hash the string secret to [u8; 32] for PoW
        let mut hasher = Sha3_256::new();
        hasher.update(pow_secret.as_bytes());
        let secret_bytes: [u8; 32] = hasher.finalize().into();

        let pow = PoW::new(secret_bytes, pow_config);

        Server {
            mint_state,
            pow,
            api_keys: api_keys.iter().cloned().collect::<HashSet<_>>(),
        }
    }

    pub fn router(self) -> Router {
        Router::new()
                .route("/api/pow", get(get_pow))
                // TODO: This feels rather ugly, and would be nice to move but I can't figure out the types.
                .route(
                    "/api/get_tokens",
                    get(get_tokens)
                        .route_layer(
                            ServiceBuilder::new()
                                .layer(
                                    // The other routes are serving static files and are therefore less interesting to log.
                                    TraceLayer::new_for_http()
                                        // Pre-register the account and amount so we can fill them in in the request.
                                        //
                                        // TODO: switch input from json to query params so we can fill in here.
                                        .make_span_with(|_request: &Request<_>| {
                                            use tracing::field::Empty;
                                            tracing::info_span!(
                                                "token_request",
                                                account = Empty,
                                                note_type = Empty,
                                                amount = Empty
                                            )
                                        })
                                        .on_response(DefaultOnResponse::new().level(Level::INFO))
                                        // Disable failure logs since we already trace errors in the method.
                                        .on_failure(())
                                ))
                )
                .layer(
                    ServiceBuilder::new()
                        .layer(SetResponseHeaderLayer::if_not_present(
                            http::header::CACHE_CONTROL,
                            HeaderValue::from_static("no-cache"),
                        ))
                        .layer(
                            CorsLayer::new()
                                .allow_origin(tower_http::cors::Any)
                                .allow_methods(tower_http::cors::Any)
                                .allow_headers([http::header::CONTENT_TYPE]),
                        ),
                )
                .with_state(self)
    }

    /// Submits a challenge to the `PoW` instance.
    ///
    /// The challenge is validated and added to the cache.
    ///
    /// # Errors
    /// Returns an error if:
    /// * The challenge is expired.
    /// * The challenge is invalid.
    /// * The challenge was already used.
    ///
    /// # Panics
    /// Panics if the current timestamp is before the UNIX epoch.
    pub(crate) fn submit_challenge(
        &self,
        challenge: &str,
        nonce: u64,
        account_id: AccountId,
        api_key: &ApiKey,
    ) -> Result<(), MintRequestError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current timestamp should be greater than unix epoch")
            .as_secs();
        self.pow.submit_challenge(account_id, api_key, challenge, nonce, timestamp)
    }
}

impl FromRef<Server> for GetTokensState {
    fn from_ref(input: &Server) -> Self {
        input.mint_state.clone()
    }
}

impl FromRef<Server> for PoW {
    fn from_ref(input: &Server) -> Self {
        // Clone is cheap: only copies a 32-byte array and increments Arc reference counters.
        input.pow.clone()
    }
}
