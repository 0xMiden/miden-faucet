//! This file explicitly embeds each of the frontend files into the binary using `include_str!` and
//! `include_bytes!`.
use std::sync::LazyLock;

use anyhow::Context;
use axum::extract::Request;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::response::Css;
use base64::Engine;
use http::header::{self};
use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tracing::info;
use url::Url;

use crate::COMPONENT;

/// Serves the frontend API endpoints.
pub async fn serve_frontend(url: Url, api_public_url: Url, node_url: String) -> anyhow::Result<()> {
    let config_json = Json(serde_json::json!({
        "api_url": api_public_url.to_string().trim_end_matches('/'),
        "node_url": node_url.trim_end_matches('/'),
    }));

    let app = Router::new()
        .route("/", get(get_index_html))
        .route("/bundle.js", get(get_bundle_js))
        .route("/index.css", get(get_index_css))
        .route("/wallet-icon.png", get(get_wallet_icon))
        .route("/header.webp", get(get_header))
        .route("/favicon.ico", get(get_favicon))
        .route("/assets/miden_client_web.wasm", get(get_miden_client_web_wasm))
        .route("/config.json", get(config_json))
        .layer(CompressionLayer::new())
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

pub async fn get_miden_client_web_wasm(request: Request) -> Response {
    const WASM_BYTES: &[u8] = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/frontend/node_modules/@miden-sdk/miden-sdk/dist/st/assets/miden_client_web.wasm"
    ));
    const WASM_BR_BYTES: &[u8] = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/frontend/node_modules/@miden-sdk/miden-sdk/dist/st/assets/miden_client_web.wasm.br"
    ));
    static WASM_ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(WASM_BYTES));
    static WASM_BR_ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(WASM_BR_BYTES));

    let mut response = if accepts_brotli(&request) {
        let mut response =
            static_response(&request, "application/wasm", WASM_BR_BYTES, &WASM_BR_ETAG);
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, HeaderValue::from_static("br"));
        response
    } else {
        // The compression layer gzips this on the fly for clients without brotli support.
        static_response(&request, "application/wasm", WASM_BYTES, &WASM_ETAG)
    };

    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("accept-encoding"));
    response
}

pub async fn get_not_found_html() -> Html<&'static str> {
    Html(include_str!(concat!(env!("OUT_DIR"), "/frontend/not_found.html")))
}

pub async fn get_bundle_js(request: Request) -> Response {
    const BUNDLE_BYTES: &[u8] =
        include_str!(concat!(env!("OUT_DIR"), "/frontend/bundle.js")).as_bytes();
    static ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(BUNDLE_BYTES));

    static_response(&request, "application/javascript", BUNDLE_BYTES, &ETAG)
}

pub async fn get_index_css() -> Css<&'static str> {
    Css(include_str!(concat!(env!("OUT_DIR"), "/frontend/index.css")))
}

pub async fn get_wallet_icon(request: Request) -> Response {
    const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/frontend/wallet-icon.png"));
    static ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(BYTES));

    static_response(&request, "image/png", BYTES, &ETAG)
}

pub async fn get_header(request: Request) -> Response {
    const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/frontend/header.webp"));
    static ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(BYTES));

    static_response(&request, "image/webp", BYTES, &ETAG)
}

pub async fn get_favicon(request: Request) -> Response {
    const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/frontend/favicon.ico"));
    static ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(BYTES));

    static_response(&request, "image/x-icon", BYTES, &ETAG)
}

/// Build a cacheable response for an embedded static asset, answering with 304 Not Modified
/// when the client already holds the current version.
fn static_response(
    request: &Request,
    content_type: &'static str,
    bytes: &'static [u8],
    etag: &str,
) -> Response {
    if let Some(response) = check_if_none_match(request, etag) {
        return response;
    }

    let mut response =
        ([(header::CONTENT_TYPE, HeaderValue::from_static(content_type))], bytes).into_response();

    add_cache_headers(response.headers_mut(), etag);
    response
}

/// Whether the client advertises brotli support in its `Accept-Encoding` header.
fn accepts_brotli(request: &Request) -> bool {
    request
        .headers()
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|encodings| {
            encodings.split(',').any(|encoding| {
                let mut parts = encoding.trim().split(';');
                let name = parts.next().unwrap_or_default().trim();
                let rejected = parts.any(|param| param.trim().replace(' ', "") == "q=0");
                name.eq_ignore_ascii_case("br") && !rejected
            })
        })
}

// CACHE HEADERS HELPERS
// ================================================================================================

/// Add `ETag` and `Cache-Control` headers to the response.
fn add_cache_headers(headers: &mut HeaderMap, etag: &str) {
    headers.insert(header::ETAG, header::HeaderValue::from_str(etag).unwrap());
    let (name, value) = cache_control_header();
    headers.insert(name, value);
}

/// Return the `Cache-Control` header with 1 day expiration.
fn cache_control_header() -> (HeaderName, HeaderValue) {
    (
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("public, max-age=86400, immutable"),
    )
}

/// Compute the `ETag` for the given byte content using SHA-256 hash and base64 encoding.
fn compute_etag(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let hash = hasher.finalize();
    format!("\"{}\"", base64::engine::general_purpose::STANDARD.encode(&hash[..16]))
}

/// Check If-None-Match header and return 304 if `ETag` matches, otherwise return None
fn check_if_none_match(request: &Request, etag: &str) -> Option<Response> {
    if let Some(if_none_match) = request.headers().get(header::IF_NONE_MATCH)
        && if_none_match.to_str().unwrap_or("") == etag
    {
        return Some(
            (
                StatusCode::NOT_MODIFIED,
                [
                    (header::ETAG, header::HeaderValue::from_str(etag).unwrap()),
                    cache_control_header(),
                ],
            )
                .into_response(),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use axum::body::Body;

    use super::*;

    fn request(headers: &[(HeaderName, &str)]) -> Request {
        let mut builder = Request::builder().uri("/assets/miden_client_web.wasm");
        for (name, value) in headers {
            builder = builder.header(name, *value);
        }
        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn wasm_is_served_brotli_compressed_when_accepted() {
        let response = get_miden_client_web_wasm(request(&[(
            header::ACCEPT_ENCODING,
            "gzip, deflate, br, zstd",
        )]))
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(header::CONTENT_ENCODING).unwrap(), "br");
        assert_eq!(response.headers().get(header::CONTENT_TYPE).unwrap(), "application/wasm");
        assert_eq!(response.headers().get(header::VARY).unwrap(), "accept-encoding");
        assert!(response.headers().contains_key(header::ETAG));
        assert!(response.headers().contains_key(header::CACHE_CONTROL));
    }

    #[tokio::test]
    async fn wasm_is_served_identity_without_brotli_support() {
        let response =
            get_miden_client_web_wasm(request(&[(header::ACCEPT_ENCODING, "gzip, deflate")])).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::CONTENT_ENCODING).is_none());
    }

    #[tokio::test]
    async fn wasm_brotli_is_rejected_with_zero_quality() {
        let response =
            get_miden_client_web_wasm(request(&[(header::ACCEPT_ENCODING, "gzip, br;q=0")])).await;

        assert!(response.headers().get(header::CONTENT_ENCODING).is_none());
    }

    #[tokio::test]
    async fn wasm_etag_roundtrip_returns_not_modified() {
        let response = get_miden_client_web_wasm(request(&[(
            header::ACCEPT_ENCODING,
            "gzip, deflate, br, zstd",
        )]))
        .await;
        let etag = response.headers().get(header::ETAG).unwrap().to_str().unwrap().to_owned();

        let revalidation = get_miden_client_web_wasm(request(&[
            (header::ACCEPT_ENCODING, "gzip, deflate, br, zstd"),
            (header::IF_NONE_MATCH, &etag),
        ]))
        .await;

        assert_eq!(revalidation.status(), StatusCode::NOT_MODIFIED);
    }
}
