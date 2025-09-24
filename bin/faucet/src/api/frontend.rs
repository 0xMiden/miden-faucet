//! This file explicitly embeds each of the frontend files into the binary using `include_str!` and
//! `include_bytes!`.

use axum::response::{Html, IntoResponse, Response};
use axum_extra::response::{Css, JavaScript};
use http::header::{self};

pub async fn get_index_html() -> Html<&'static str> {
    Html(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend/index.html")))
}

pub async fn get_not_found_html() -> Html<&'static str> {
    Html(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend/not_found.html")))
}

pub async fn get_bundle_js() -> JavaScript<&'static str> {
    JavaScript(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend/bundle.js")))
}

pub async fn get_index_css() -> Css<&'static str> {
    Css(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend/index.css")))
}

pub async fn get_background() -> Response {
    (
        [(header::CONTENT_TYPE, header::HeaderValue::from_static("image/png"))],
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend/background.png"),),
    )
        .into_response()
}

pub async fn get_wallet_icon() -> Response {
    (
        [(header::CONTENT_TYPE, header::HeaderValue::from_static("image/png"))],
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend/wallet-icon.png"),),
    )
        .into_response()
}

pub async fn get_favicon() -> Response {
    (
        [(header::CONTENT_TYPE, header::HeaderValue::from_static("image/x-icon"))],
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend/favicon.ico"),),
    )
        .into_response()
}
