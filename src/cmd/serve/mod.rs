mod browse;
pub mod config;
mod git;
mod handlers;
mod landing;
pub mod server;
mod state;

#[cfg(test)]
mod tests {
    use std::fs::write;

    use portpicker::pick_unused_port;
    use tempfile::tempdir;
    use tokio::spawn;

    use crate::cmd::serve::config::DefaultsSection;
    use crate::cmd::serve::config::ResolvedCollection;
    use crate::cmd::serve::config::ResolvedServeConfig;
    use crate::cmd::serve::server::start_serve;
    use crate::error::Fallible;
    use crate::utils::wait_for_server;

    const TEST_HOST: &str = "127.0.0.1";

    /// Regression test: POSTing multiple `decks` values to /collection/{slug}/start
    /// must not fail with "duplicate field `decks`".
    #[tokio::test]
    async fn test_start_with_multiple_decks() -> Fallible<()> {
        let port = pick_unused_port().unwrap();
        let dir = tempdir()?;
        let coll_dir = dir.path().to_path_buf();

        // Create two markdown files representing two different decks.
        write(
            coll_dir.join("Alpha.md"),
            "Q: What is 1+1?\nA: 2\n",
        )?;
        write(
            coll_dir.join("Beta.md"),
            "Q: What is 2+2?\nA: 4\n",
        )?;

        let slug = "test-collection".to_string();
        let config = ResolvedServeConfig {
            host: TEST_HOST.to_string(),
            port,
            git: None,
            defaults: DefaultsSection::default(),
            collections: vec![ResolvedCollection {
                name: "Test Collection".to_string(),
                slug: slug.clone(),
                coll_dir: coll_dir.clone(),
                db_path: coll_dir.join("hashcards.db"),
            }],
        };

        spawn(async move { start_serve(config).await });
        wait_for_server(TEST_HOST, port).await?;

        // POST with multiple `decks` values — this used to fail with
        // "Failed to deserialize form body: duplicate field `decks`".
        let response = reqwest::Client::new()
            .post(format!("http://{TEST_HOST}:{port}/collection/{slug}/start"))
            .body("decks=Alpha&decks=Beta")
            .header("content-type", "application/x-www-form-urlencoded")
            .send()
            .await?;

        // The handler redirects on success; reqwest follows redirects by
        // default, so any 2xx status means the form was accepted.
        assert!(
            response.status().is_success(),
            "expected success, got {}",
            response.status()
        );

        Ok(())
    }
}
