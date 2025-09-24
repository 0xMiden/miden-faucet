use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use axum::Router;
use axum::extract::FromRef;
use axum::routing::get;
use http::{HeaderValue, Request};
use miden_client::account::{AccountId, AccountIdError, AddressError};
use miden_client::store::Store;
use miden_client::utils::{RwLock, hex_to_bytes};
use miden_faucet_lib::FaucetId;
use miden_faucet_lib::requests::MintRequestSender;
use miden_faucet_lib::types::AssetAmount;
use miden_pow_rate_limiter::{Challenge, ChallengeError, PoWRateLimiter, PoWRateLimiterConfig};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;
use url::Url;

use crate::COMPONENT;
use crate::api::get_metadata::{Metadata, get_metadata};
use crate::api::get_note::get_note;
use crate::api::get_pow::get_pow;
use crate::api::get_tokens::{GetTokensState, MintRequestError, get_tokens};
use crate::api_key::ApiKey;

mod frontend;
mod get_metadata;
mod get_note;
mod get_pow;
mod get_tokens;

// FAUCET STATE
// ================================================================================================

/// Serves the faucet's website and handles token requests.
#[derive(Clone)]
pub struct Server {
    mint_state: GetTokensState,
    metadata: &'static Metadata,
    rate_limiter: PoWRateLimiter,
    api_keys: HashSet<ApiKey>,
    store: Arc<dyn Store>,
}

impl Server {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        faucet_id: FaucetId,
        decimals: u8,
        max_supply: AssetAmount,
        issuance: Arc<RwLock<AssetAmount>>,
        max_claimable_amount: AssetAmount,
        mint_request_sender: MintRequestSender,
        pow_secret: &str,
        rate_limiter_config: PoWRateLimiterConfig,
        api_keys: &[ApiKey],
        store: Arc<dyn Store>,
        explorer_url: Option<Url>,
    ) -> Self {
        let mint_state = GetTokensState::new(mint_request_sender, max_claimable_amount);
        let metadata = Metadata {
            id: faucet_id,
            issuance,
            max_supply,
            decimals,
            explorer_url,
        };
        // SAFETY: Leaking is okay because we want it to live as long as the application.
        let metadata = Box::leak(Box::new(metadata));

        // Hash the string secret to [u8; 32] for PoW
        let mut hasher = Sha256::new();
        hasher.update(pow_secret.as_bytes());
        let secret_bytes: [u8; 32] = hasher.finalize().into();

        let rate_limiter = PoWRateLimiter::new(secret_bytes, rate_limiter_config);

        Server {
            mint_state,
            metadata,
            rate_limiter,
            api_keys: api_keys.iter().cloned().collect::<HashSet<_>>(),
            store,
        }
    }

    #[allow(clippy::too_many_lines)]
    pub async fn serve(self, url: Url) -> anyhow::Result<()> {
        let app = Router::new()
                .route("/", get(frontend::get_index_html))
                .route("/index.js", get(frontend::get_index_js))
                .route("/index.css", get(frontend::get_index_css))
                .route("/background.png", get(frontend::get_background))
                .route("/favicon.ico", get(frontend::get_favicon))
                .fallback(get(frontend::get_not_found_html))
                .route("/get_metadata", get(get_metadata))
                .route("/pow", get(get_pow))
                // TODO: This feels rather ugly, and would be nice to move but I can't figure out the types.
                .route(
                    "/get_tokens",
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
                .route("/get_note", get(get_note))
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
                .with_state(self);

        let listener = url
            .socket_addrs(|| None)
            .with_context(|| format!("failed to parse url {url}"))?;
        let listener = TcpListener::bind(&*listener)
            .await
            .with_context(|| format!("failed to bind TCP listener on {url}"))?;

        tracing::info!(target: COMPONENT, address = %url, "Server started");

        axum::serve(listener, app).await.map_err(Into::into)
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
        api_key: ApiKey,
    ) -> Result<(), MintRequestError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current timestamp should be greater than unix epoch")
            .as_secs();
        let account_id_bytes: [u8; AccountId::SERIALIZED_SIZE] = account_id.into();
        let mut requestor = [0u8; 32];
        requestor[..AccountId::SERIALIZED_SIZE].copy_from_slice(&account_id_bytes);

        let challenge = hex_to_bytes::<{ Challenge::SERIALIZED_SIZE }>(challenge)
            .map_err(|_| MintRequestError::PowError(ChallengeError::InvalidSerialization))?;
        self.rate_limiter
            .submit_challenge(requestor, api_key, &challenge, nonce, timestamp)
            .map_err(MintRequestError::PowError)
    }
}

impl FromRef<Server> for &'static Metadata {
    fn from_ref(input: &Server) -> Self {
        input.metadata
    }
}

impl FromRef<Server> for GetTokensState {
    fn from_ref(input: &Server) -> Self {
        input.mint_state.clone()
    }
}

impl FromRef<Server> for PoWRateLimiter {
    fn from_ref(input: &Server) -> Self {
        // Clone is cheap: only copies a 32-byte array and increments Arc reference counters.
        input.rate_limiter.clone()
    }
}

// ERRORS
// ================================================================================================

/// Errors that can occur when parsing an account ID or address.
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("account ID failed to parse")]
    ParseId(#[source] AccountIdError),
    #[error("account address failed to parse")]
    ParseAddress(#[source] AddressError),
    #[error("account address is not an ID based")]
    AddressNotIdBased,
}
