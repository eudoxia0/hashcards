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

mod get;
mod post;
pub mod server;
mod state;
mod template;

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::path::PathBuf;
    use std::time::Duration;

    use reqwest::StatusCode;
    use serial_test::serial;
    use tokio::fs::remove_file;
    use tokio::net::TcpStream;
    use tokio::spawn;
    use tokio::time::sleep;

    use crate::drill::server::start_server;
    use crate::error::Fallible;
    use crate::types::timestamp::Timestamp;

    #[tokio::test]
    async fn test_start_server_on_non_existent_directory() -> Fallible<()> {
        let directory = PathBuf::from("./derpherp");
        let session_started_at = Timestamp::now();
        let result = start_server(directory, session_started_at).await;
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.to_string(), "error: directory does not exist.");
        Ok(())
    }

    #[tokio::test]
    async fn test_start_server_on_empty_directory() -> Fallible<()> {
        let directory = temp_dir();
        let session_started_at = Timestamp::now();
        let result = start_server(directory, session_started_at).await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_e2e() -> Fallible<()> {
        let directory = PathBuf::from("./test").canonicalize().unwrap();
        let db_path = directory.join("db.sqlite3");
        if db_path.exists() {
            remove_file(&db_path).await?;
        }

        let session_started_at = Timestamp::now();
        spawn(async move { start_server(directory, session_started_at).await });
        loop {
            if let Ok(stream) = TcpStream::connect("0.0.0.0:8000").await {
                drop(stream);
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }

        // Hit the `style.css` endpoint.
        let response = reqwest::get("http://0.0.0.0:8000/style.css").await?;
        assert!(response.status().is_success());
        assert_eq!(response.headers().get("content-type").unwrap(), "text/css");

        // Hit the `script.js` endpoint.
        let response = reqwest::get("http://0.0.0.0:8000/script.js").await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/javascript"
        );

        // Hit the not found endpoint.
        let response = reqwest::get("http://0.0.0.0:8000/herp-derp").await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Hit the image endpoint.
        let response = reqwest::get("http://0.0.0.0:8000/image/foo.jpg").await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );

        // Hit the image endpoint with a non-existent image.
        let response = reqwest::get("http://0.0.0.0:8000/image/foo.png").await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Hit the root endpoint.
        let response = reqwest::get("http://0.0.0.0:8000/").await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
        let html = response.text().await?;
        assert!(html.contains("baz <span class='cloze'>.............</span>"));

        // Hit reveal.
        let response = reqwest::Client::new()
            .post("http://0.0.0.0:8000/")
            .form(&[("action", "Reveal")])
            .send()
            .await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
        let html = response.text().await?;
        assert!(html.contains("baz <span class='cloze-reveal'>quux</span>"));

        // Hit 'Good'.
        let response = reqwest::Client::new()
            .post("http://0.0.0.0:8000/")
            .form(&[("action", "Good")])
            .send()
            .await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
        let html = response.text().await?;
        assert!(html.contains("FOO"));

        // Hit reveal.
        let response = reqwest::Client::new()
            .post("http://0.0.0.0:8000/")
            .form(&[("action", "Reveal")])
            .send()
            .await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
        let html = response.text().await?;
        assert!(html.contains("BAR"));

        // Hit 'Good'.
        let response = reqwest::Client::new()
            .post("http://0.0.0.0:8000/")
            .form(&[("action", "Good")])
            .send()
            .await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
        let html = response.text().await?;
        assert!(html.contains("Session Completed"));

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_undo() -> Fallible<()> {
        let directory = PathBuf::from("./test").canonicalize().unwrap();
        let db_path = directory.join("db.sqlite3");
        if db_path.exists() {
            remove_file(&db_path).await?;
        }

        // Start the server
        let session_started_at = Timestamp::now();
        spawn(async move { start_server(directory, session_started_at).await });
        loop {
            if let Ok(stream) = TcpStream::connect("0.0.0.0:8000").await {
                drop(stream);
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }

        // Hit reveal.
        let response = reqwest::Client::new()
            .post("http://0.0.0.0:8000/")
            .form(&[("action", "Reveal")])
            .send()
            .await?;
        assert!(response.status().is_success());

        // Hit 'Good'.
        let response = reqwest::Client::new()
            .post("http://0.0.0.0:8000/")
            .form(&[("action", "Good")])
            .send()
            .await?;
        assert!(response.status().is_success());

        // Hit undo.
        let response = reqwest::Client::new()
            .post("http://0.0.0.0:8000/")
            .form(&[("action", "Undo")])
            .send()
            .await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
        let html = response.text().await?;
        assert!(html.contains("baz <span class='cloze'>.............</span>"));

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_end() -> Fallible<()> {
        let directory = PathBuf::from("./test").canonicalize().unwrap();
        let db_path = directory.join("db.sqlite3");
        if db_path.exists() {
            remove_file(&db_path).await?;
        }

        // Start the server
        let session_started_at = Timestamp::now();
        spawn(async move { start_server(directory, session_started_at).await });
        loop {
            if let Ok(stream) = TcpStream::connect("0.0.0.0:8000").await {
                drop(stream);
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }

        // Hit end.
        let response = reqwest::Client::new()
            .post("http://0.0.0.0:8000/")
            .form(&[("action", "End")])
            .send()
            .await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
        let html = response.text().await?;
        assert!(html.contains("Session Completed"));

        Ok(())
    }
}
