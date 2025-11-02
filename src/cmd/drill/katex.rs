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

pub const KATEX_JS_URL: &str = "/katex/katex.js";
pub const KATEX_CSS_URL: &str = "/katex/katex.css";
pub const KATEX_AUTO_RENDER_JS_URL: &str = "/katex/katex-auto-render.js";

pub async fn katex_css_handler() -> (StatusCode, [(HeaderName, &'static str); 2], String) {
    let css = include_str!("../../../vendor/katex/katex.min.css");
    // Rewrite font URLs from "fonts/" to "/katex/fonts/"
    let css = css.replace("fonts/", "/katex/fonts/");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/css"),
            (CACHE_CONTROL, "public, max-age=604800, immutable"),
        ],
        css,
    )
}

pub async fn katex_js_handler() -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    let bytes = include_bytes!("../../../vendor/katex/katex.min.js");
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/javascript"),
            (CACHE_CONTROL, "public, max-age=604800, immutable"),
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
            (CACHE_CONTROL, "public, max-age=604800, immutable"),
        ],
        bytes,
    )
}

pub async fn katex_font_handler(
    Path(path): Path<String>,
) -> (StatusCode, [(HeaderName, &'static str); 2], &'static [u8]) {
    // Determine content type based on file extension
    let content_type = if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else {
        return (
            StatusCode::NOT_FOUND,
            [(CONTENT_TYPE, "text/plain"), (CACHE_CONTROL, "no-cache")],
            b"Not Found",
        );
    };

    // Match font files
    let bytes: &'static [u8] = match path.as_str() {
        "KaTeX_AMS-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_AMS-Regular.woff2")
        }
        "KaTeX_AMS-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_AMS-Regular.woff")
        }
        "KaTeX_AMS-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_AMS-Regular.ttf")
        }
        "KaTeX_Caligraphic-Bold.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Caligraphic-Bold.woff2")
        }
        "KaTeX_Caligraphic-Bold.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Caligraphic-Bold.woff")
        }
        "KaTeX_Caligraphic-Bold.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Caligraphic-Bold.ttf")
        }
        "KaTeX_Caligraphic-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Caligraphic-Regular.woff2")
        }
        "KaTeX_Caligraphic-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Caligraphic-Regular.woff")
        }
        "KaTeX_Caligraphic-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Caligraphic-Regular.ttf")
        }
        "KaTeX_Fraktur-Bold.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Fraktur-Bold.woff2")
        }
        "KaTeX_Fraktur-Bold.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Fraktur-Bold.woff")
        }
        "KaTeX_Fraktur-Bold.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Fraktur-Bold.ttf")
        }
        "KaTeX_Fraktur-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Fraktur-Regular.woff2")
        }
        "KaTeX_Fraktur-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Fraktur-Regular.woff")
        }
        "KaTeX_Fraktur-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Fraktur-Regular.ttf")
        }
        "KaTeX_Main-Bold.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Bold.woff2")
        }
        "KaTeX_Main-Bold.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Bold.woff")
        }
        "KaTeX_Main-Bold.ttf" => include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Bold.ttf"),
        "KaTeX_Main-BoldItalic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-BoldItalic.woff2")
        }
        "KaTeX_Main-BoldItalic.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-BoldItalic.woff")
        }
        "KaTeX_Main-BoldItalic.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-BoldItalic.ttf")
        }
        "KaTeX_Main-Italic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Italic.woff2")
        }
        "KaTeX_Main-Italic.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Italic.woff")
        }
        "KaTeX_Main-Italic.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Italic.ttf")
        }
        "KaTeX_Main-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Regular.woff2")
        }
        "KaTeX_Main-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Regular.woff")
        }
        "KaTeX_Main-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Main-Regular.ttf")
        }
        "KaTeX_Math-BoldItalic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Math-BoldItalic.woff2")
        }
        "KaTeX_Math-BoldItalic.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Math-BoldItalic.woff")
        }
        "KaTeX_Math-BoldItalic.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Math-BoldItalic.ttf")
        }
        "KaTeX_Math-Italic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Math-Italic.woff2")
        }
        "KaTeX_Math-Italic.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Math-Italic.woff")
        }
        "KaTeX_Math-Italic.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Math-Italic.ttf")
        }
        "KaTeX_SansSerif-Bold.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Bold.woff2")
        }
        "KaTeX_SansSerif-Bold.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Bold.woff")
        }
        "KaTeX_SansSerif-Bold.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Bold.ttf")
        }
        "KaTeX_SansSerif-Italic.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Italic.woff2")
        }
        "KaTeX_SansSerif-Italic.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Italic.woff")
        }
        "KaTeX_SansSerif-Italic.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Italic.ttf")
        }
        "KaTeX_SansSerif-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Regular.woff2")
        }
        "KaTeX_SansSerif-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Regular.woff")
        }
        "KaTeX_SansSerif-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_SansSerif-Regular.ttf")
        }
        "KaTeX_Script-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Script-Regular.woff2")
        }
        "KaTeX_Script-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Script-Regular.woff")
        }
        "KaTeX_Script-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Script-Regular.ttf")
        }
        "KaTeX_Size1-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size1-Regular.woff2")
        }
        "KaTeX_Size1-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size1-Regular.woff")
        }
        "KaTeX_Size1-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size1-Regular.ttf")
        }
        "KaTeX_Size2-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size2-Regular.woff2")
        }
        "KaTeX_Size2-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size2-Regular.woff")
        }
        "KaTeX_Size2-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size2-Regular.ttf")
        }
        "KaTeX_Size3-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size3-Regular.woff2")
        }
        "KaTeX_Size3-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size3-Regular.woff")
        }
        "KaTeX_Size3-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size3-Regular.ttf")
        }
        "KaTeX_Size4-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size4-Regular.woff2")
        }
        "KaTeX_Size4-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size4-Regular.woff")
        }
        "KaTeX_Size4-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Size4-Regular.ttf")
        }
        "KaTeX_Typewriter-Regular.woff2" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Typewriter-Regular.woff2")
        }
        "KaTeX_Typewriter-Regular.woff" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Typewriter-Regular.woff")
        }
        "KaTeX_Typewriter-Regular.ttf" => {
            include_bytes!("../../../vendor/katex/fonts/KaTeX_Typewriter-Regular.ttf")
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
            (CONTENT_TYPE, content_type),
            (CACHE_CONTROL, "public, max-age=604800, immutable"),
        ],
        bytes,
    )
}
