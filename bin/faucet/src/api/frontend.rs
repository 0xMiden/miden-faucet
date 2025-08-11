use axum::response::{Html, IntoResponse, Response};
use axum_extra::response::{Css, JavaScript};
use http::header::{self};

pub async fn get_index_html() -> Html<&'static str> {
    Html(include_str!("../../frontend/index.html"))
}

pub async fn get_index_js() -> JavaScript<&'static str> {
    JavaScript(include_str!("../../frontend/index.js"))
}

pub async fn get_index_css() -> Css<&'static str> {
    Css(include_str!("../../frontend/index.css"))
}

pub async fn get_background() -> Response {
    (
        [(header::CONTENT_TYPE, header::HeaderValue::from_static("image/png"))],
        include_bytes!("../../frontend/background.png"),
    )
        .into_response()
}

pub async fn get_favicon() -> Response {
    (
        [(header::CONTENT_TYPE, header::HeaderValue::from_static("image/x-icon"))],
        include_bytes!("../../frontend/favicon.ico"),
    )
        .into_response()
}
