// Copyright 2025 Fernando Borretti
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

use axum::extract::Path;
use axum::http::HeaderName;
use axum::http::StatusCode;
use axum::http::header::CACHE_CONTROL;
use axum::http::header::CONTENT_TYPE;

use crate::utils::CACHE_CONTROL_IMMUTABLE;

pub const KATEX_JS_URL: &str = "/katex/katex.js";
pub const KATEX_CSS_URL: &str = "/katex/katex.css";
pub const KATEX_AUTO_RENDER_JS_URL: &str = "/katex/katex-auto-render.js";

pub async fn katex_css_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    let bytes = include_bytes!("../../../vendor/katex/katex.min.css");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/css"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}

pub async fn katex_js_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    let bytes = include_bytes!("../../../vendor/katex/katex.min.js");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/javascript"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}

pub async fn katex_auto_render_handler()
-> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    let bytes = include_bytes!("../../../vendor/katex/contrib/auto-render.min.js");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/javascript"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}

pub async fn katex_font_handler(
    Path(path): Path<String>,
) -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    // Only serve WOFF2 fonts (modern format with best compression and 95%+ browser support)
    if !path.ends_with(".woff2") {
        return (
            StatusCode::NOT_FOUND,
            [(CONTENT_TYPE, "text/plain"), (CACHE_CONTROL, "no-cache")],
            b"Not Found",
        );
    }

    // Match font files (WOFF2 only)
    let bytes: &'static [u8] = match path.as_str() {
        "KaTeX_AMS-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_AMS-Regular.woff2")
        }
        "KaTeX_Caligraphic-Bold.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Caligraphic-Bold.woff2")
        }
        "KaTeX_Caligraphic-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Caligraphic-Regular.woff2")
        }
        "KaTeX_Fraktur-Bold.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Fraktur-Bold.woff2")
        }
        "KaTeX_Fraktur-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Fraktur-Regular.woff2")
        }
        "KaTeX_Main-Bold.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Bold.woff2")
        }
        "KaTeX_Main-BoldItalic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-BoldItalic.woff2")
        }
        "KaTeX_Main-Italic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Italic.woff2")
        }
        "KaTeX_Main-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Regular.woff2")
        }
        "KaTeX_Math-BoldItalic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Math-BoldItalic.woff2")
        }
        "KaTeX_Math-Italic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Math-Italic.woff2")
        }
        "KaTeX_SansSerif-Bold.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Bold.woff2")
        }
        "KaTeX_SansSerif-Italic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Italic.woff2")
        }
        "KaTeX_SansSerif-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Regular.woff2")
        }
        "KaTeX_Script-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Script-Regular.woff2")
        }
        "KaTeX_Size1-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size1-Regular.woff2")
        }
        "KaTeX_Size2-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size2-Regular.woff2")
        }
        "KaTeX_Size3-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size3-Regular.woff2")
        }
        "KaTeX_Size4-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size4-Regular.woff2")
        }
        "KaTeX_Typewriter-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Typewriter-Regular.woff2")
        }
        _ => {
            return (
                StatusCode::NOT_FOUND,
                [(CONTENT_TYPE, "text/plain"), (CACHE_CONTROL, "no-cache")],
                b"Not Found",
            );
        }
    };

    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "font/woff2"),
            (CACHE_CONTROL, CACHE_CONTROL_IMMUTABLE),
        ],
        bytes,
    )
}
