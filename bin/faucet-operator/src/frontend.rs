//! This file explicitly embeds each of the frontend files into the binary using `include_str!` and
//! `include_bytes!`.
use anyhow::Context;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::response::{Css, JavaScript};
use http::header::{self};
use tokio::net::TcpListener;
use tracing::info;
use url::Url;

use crate::COMPONENT;

/// Serves the frontend API endpoints.
pub async fn serve_frontend(url: Url, api_public_url: Url, node_url: String) -> anyhow::Result<()> {
    let config_json = Json(
        serde_json::json!({
            "api_url": api_public_url.to_string().trim_end_matches('/'),
            "node_url": node_url.trim_end_matches('/'),
        })
        .to_string(),
    );

    let app = Router::new()
        .route("/", get(get_index_html))
        .route("/bundle.js", get(get_bundle_js))
        .route("/index.css", get(get_index_css))
        .route("/background.png", get(get_background))
        .route("/wallet-icon.png", get(get_wallet_icon))
        .route("/favicon.ico", get(get_favicon))
        .route("/config.json", get(config_json))
        .fallback(get(get_not_found_html));

    let listener = url
        .socket_addrs(|| None)
        .with_context(|| format!("failed to parse url {url}"))?;
    let listener = TcpListener::bind(&*listener)
        .await
        .with_context(|| format!("failed to bind TCP listener on {url}"))?;

    info!(target: COMPONENT, address = %url, "Frontend server started");

    axum::serve(listener, app).await.map_err(Into::into)
}

pub async fn get_index_html() -> Html<&'static str> {
    Html(include_str!(concat!(env!("OUT_DIR"), "/frontend/index.html")))
}

pub async fn get_not_found_html() -> Html<&'static str> {
    Html(include_str!(concat!(env!("OUT_DIR"), "/frontend/not_found.html")))
}

pub async fn get_bundle_js() -> JavaScript<&'static str> {
    JavaScript(include_str!(concat!(env!("OUT_DIR"), "/frontend/bundle.js")))
}

pub async fn get_index_css() -> Css<&'static str> {
    Css(include_str!(concat!(env!("OUT_DIR"), "/frontend/index.css")))
}

pub async fn get_background() -> Response {
    (
        [(header::CONTENT_TYPE, header::HeaderValue::from_static("image/png"))],
        include_bytes!(concat!(env!("OUT_DIR"), "/frontend/background.png"),),
    )
        .into_response()
}

pub async fn get_wallet_icon() -> Response {
    (
        [(header::CONTENT_TYPE, header::HeaderValue::from_static("image/png"))],
        include_bytes!(concat!(env!("OUT_DIR"), "/frontend/wallet-icon.png"),),
    )
        .into_response()
}

pub async fn get_favicon() -> Response {
    (
        [(header::CONTENT_TYPE, header::HeaderValue::from_static("image/x-icon"))],
        include_bytes!(concat!(env!("OUT_DIR"), "/frontend/favicon.ico"),),
    )
        .into_response()
}
