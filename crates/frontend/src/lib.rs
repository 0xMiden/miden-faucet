use axum::{Router, extract::FromRef, routing::get};
use frontend::Metadata;
use miden_faucet_common::{AssetOptions, FaucetId};

mod frontend;

// FRONTEND SERVER
// ================================================================================================

/// Serves the faucet's website frontend.
#[derive(Clone)]
pub struct Server {
    metadata: &'static Metadata,
}

impl Server {
    pub fn new(faucet_id: FaucetId, asset_options: AssetOptions) -> Self {
        let metadata = Metadata {
            id: faucet_id,
            asset_amount_options: asset_options,
        };
        // SAFETY: Leaking is okay because we want it to live as long as the application.
        let metadata = Box::leak(Box::new(metadata));

        Server { metadata }
    }

    pub fn router(self) -> Router {
        Router::new()
            .route("/", get(frontend::get_index_html))
            .route("/index.js", get(frontend::get_index_js))
            .route("/index.css", get(frontend::get_index_css))
            .route("/background.png", get(frontend::get_background))
            .route("/favicon.ico", get(frontend::get_favicon))
            .route("/get_metadata", get(frontend::get_metadata))
            .with_state(self)
    }
}

impl FromRef<Server> for &'static Metadata {
    fn from_ref(input: &Server) -> Self {
        input.metadata
    }
}
