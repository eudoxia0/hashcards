// Copyright 2025–2026 Fernando Borretti
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::http::header::CACHE_CONTROL;
use axum::http::header::CONTENT_TYPE;

use crate::utils::CACHE_CONTROL_IMMUTABLE;

pub const HIGHLIGHT_JS_URL: &str = "/highlight.js";

pub const HIGHLIGHT_CSS_URL: &str = "/highlight.css";

pub async fn highlight_css_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8])
{
    let bytes = include_bytes!("../../../vendor/highlight/highlight.css");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/css"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}

pub async fn highlight_js_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8])
{
    let bytes = include_bytes!("../../../vendor/highlight/highlight.js");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/javascript"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}
