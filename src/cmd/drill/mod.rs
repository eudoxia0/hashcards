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

mod cache;
mod file;
mod get;
mod post;
pub mod server;
mod state;
mod template;

#[cfg(test)]
mod tests {
    use std::fs::create_dir_all;

    use portpicker::pick_unused_port;
    use reqwest::StatusCode;
    use tempfile::tempdir;
    use tokio::spawn;

    use crate::cmd::drill::server::start_server;
    use crate::error::Fallible;
    use crate::helper::create_tmp_copy_of_test_directory;
    use crate::types::timestamp::Timestamp;
    use crate::utils::wait_for_server;

    #[tokio::test]
    async fn test_start_server_on_non_existent_directory() -> Fallible<()> {
        let port = pick_unused_port().unwrap();
        let session_started_at = Timestamp::now();
        let result = start_server(
            Some("./derpherp".to_string()),
            port,
            session_started_at,
            None,
            None,
            None,
        )
        .await;
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.to_string(), "error: directory does not exist.");
        Ok(())
    }

    #[tokio::test]
    async fn test_start_server_with_no_cards_due() -> Fallible<()> {
        let port = pick_unused_port().unwrap();
        let dir = tempdir()?.path().to_path_buf().canonicalize()?;
        create_dir_all(&dir)?;
        let session_started_at = Timestamp::now();
        let dir = dir.canonicalize().unwrap().display().to_string();
        start_server(Some(dir), port, session_started_at, None, None, None).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_e2e() -> Fallible<()> {
        let port = pick_unused_port().unwrap();
        let directory = create_tmp_copy_of_test_directory()?;
        let session_started_at = Timestamp::now();
        spawn(async move {
            start_server(Some(directory), port, session_started_at, None, None, None).await
        });
        wait_for_server(port).await?;

        // Hit the `style.css` endpoint.
        let response = reqwest::get(format!("http://0.0.0.0:{port}/style.css")).await?;
        assert!(response.status().is_success());
        assert_eq!(response.headers().get("content-type").unwrap(), "text/css");

        // Hit the `script.js` endpoint.
        let response = reqwest::get(format!("http://0.0.0.0:{port}/script.js")).await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/javascript"
        );

        // Hit the not found endpoint.
        let response = reqwest::get(format!("http://0.0.0.0:{port}/herp-derp")).await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Hit the image endpoint.
        let response = reqwest::get(format!("http://0.0.0.0:{port}/image/foo.jpg")).await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );

        // Hit the image endpoint with a non-existent image.
        let response = reqwest::get(format!("http://0.0.0.0:{port}/image/foo.png")).await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Hit the root endpoint.
        let response = reqwest::get(format!("http://0.0.0.0:{port}/")).await?;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8"
        );
        let html = response.text().await?;
        assert!(html.contains("baz <span class='cloze'>.............</span>"));

        // Hit reveal.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Reveal")])
            .send()
            .await?;
        assert!(response.status().is_success());
        let html = response.text().await?;
        assert!(html.contains("baz <span class='cloze-reveal'>quux</span>"));

        // Hit 'Good'.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Good")])
            .send()
            .await?;
        assert!(response.status().is_success());
        let html = response.text().await?;
        assert!(html.contains("FOO"));

        // Hit reveal.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Reveal")])
            .send()
            .await?;
        assert!(response.status().is_success());
        let html = response.text().await?;
        assert!(html.contains("BAR"));

        // Hit 'Good'.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Good")])
            .send()
            .await?;
        assert!(response.status().is_success());
        let html = response.text().await?;
        assert!(html.contains("Session Completed"));

        Ok(())
    }

    #[tokio::test]
    async fn test_undo() -> Fallible<()> {
        let port = pick_unused_port().unwrap();
        let directory = create_tmp_copy_of_test_directory()?;
        let session_started_at = Timestamp::now();
        spawn(async move {
            start_server(Some(directory), port, session_started_at, None, None, None).await
        });
        wait_for_server(port).await?;

        // Hit reveal.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Reveal")])
            .send()
            .await?;
        assert!(response.status().is_success());

        // Hit 'Good'.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Good")])
            .send()
            .await?;
        assert!(response.status().is_success());

        // Hit undo.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Undo")])
            .send()
            .await?;
        assert!(response.status().is_success());
        let html = response.text().await?;
        assert!(html.contains("baz <span class='cloze'>.............</span>"));

        Ok(())
    }

    #[tokio::test]
    async fn test_undo_initial() -> Fallible<()> {
        let port = pick_unused_port().unwrap();
        let directory = create_tmp_copy_of_test_directory()?;
        let session_started_at = Timestamp::now();
        spawn(async move {
            start_server(Some(directory), port, session_started_at, None, None, None).await
        });
        wait_for_server(port).await?;

        // Hit undo.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Undo")])
            .send()
            .await?;
        assert!(response.status().is_success());

        Ok(())
    }

    #[tokio::test]
    async fn test_answer_without_reveal() -> Fallible<()> {
        let port = pick_unused_port().unwrap();
        let directory = create_tmp_copy_of_test_directory()?;
        let session_started_at = Timestamp::now();
        spawn(async move {
            start_server(Some(directory), port, session_started_at, None, None, None).await
        });
        wait_for_server(port).await?;

        // Hit 'Hard'.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Hard")])
            .send()
            .await?;
        assert!(response.status().is_success());

        Ok(())
    }

    #[tokio::test]
    async fn test_undo_forgetting() -> Fallible<()> {
        let port = pick_unused_port().unwrap();
        let directory = create_tmp_copy_of_test_directory()?;
        let session_started_at = Timestamp::now();
        spawn(async move {
            start_server(Some(directory), port, session_started_at, None, None, None).await
        });
        wait_for_server(port).await?;

        // Hit reveal.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Reveal")])
            .send()
            .await?;
        assert!(response.status().is_success());

        // Hit 'Forgot'.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Forgot")])
            .send()
            .await?;
        assert!(response.status().is_success());

        // Hit undo.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "Undo")])
            .send()
            .await?;
        assert!(response.status().is_success());
        let html = response.text().await?;
        assert!(html.contains("baz <span class='cloze'>.............</span>"));

        Ok(())
    }

    #[tokio::test]
    async fn test_end() -> Fallible<()> {
        let port = pick_unused_port().unwrap();
        let directory = create_tmp_copy_of_test_directory()?;
        let session_started_at = Timestamp::now();
        spawn(async move {
            start_server(Some(directory), port, session_started_at, None, None, None).await
        });
        wait_for_server(port).await?;

        // Hit end.
        let response = reqwest::Client::new()
            .post(format!("http://0.0.0.0:{port}/"))
            .form(&[("action", "End")])
            .send()
            .await?;
        assert!(response.status().is_success());
        let html = response.text().await?;
        assert!(html.contains("Session Completed"));

        Ok(())
    }
}
