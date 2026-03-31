use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::http::header::CACHE_CONTROL;
use axum::http::header::CONTENT_TYPE;

use crate::utils::CACHE_CONTROL_IMMUTABLE;

pub const HLJS_JS_URL: &str = "/hljs/highlight.js";
pub const HLJS_CSS_URL: &str = "/hljs/github.css";

pub async fn hljs_js_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    let bytes = include_bytes!("../../../vendor/highlightjs/highlight.min.js");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/javascript"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}

pub async fn hljs_css_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    let bytes = include_bytes!("../../../vendor/highlightjs/github.min.css");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/css"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}
