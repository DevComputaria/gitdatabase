use std::fs;

use anyhow::Result;
use git2::{Repository, Signature};
use gitbase_db::connect;
use gitbase_loader::{
    hydrate_blobs, index_uast, sync_repositories, BlobHydrationConfig, UastIndexConfig,
};
use sqlx::PgPool;
use tempfile::TempDir;

#[tokio::test]
async fn index_uast_populates_functions() -> Result<()> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) => value,
        Err(_) => {
            eprintln!("DATABASE_URL not set; skipping integration test");
            return Ok(());
        }
    };

    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path().join("example-repo");
    fs::create_dir_all(&repo_path)?;
    let repo = Repository::init(&repo_path)?;

    fs::write(
        repo_path.join("main.go"),
        "package main\n\nfunc main() {}\n",
    )?;
    fs::write(repo_path.join("lib.rs"), "pub fn hello() {}\n")?;

    let mut index = repo.index()?;
    index.add_path(std::path::Path::new("main.go"))?;
    index.add_path(std::path::Path::new("lib.rs"))?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let signature = Signature::now("Gitbase", "gitbase@example.com")?;
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "initial commit",
        &tree,
        &[],
    )?;

    let pool = connect(&database_url, 5).await?;
    clear_tables(&pool).await?;

    let _ = sync_repositories(&pool, &[repo_path.clone()]).await?;

    let blob_hashes: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT blob_hash FROM gitbase.files")
            .fetch_all(&pool)
            .await?;

    let _ = hydrate_blobs(
        &pool,
        &[repo_path.clone()],
        &blob_hashes,
        &BlobHydrationConfig::default(),
    )
    .await?;

    let report = index_uast(&pool, &UastIndexConfig::default()).await?;
    assert!(report.parsed >= 2);

    let fn_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gitbase.uast_functions")
        .fetch_one(&pool)
        .await?;
    assert!(fn_count >= 2);

    Ok(())
}

async fn clear_tables(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "TRUNCATE gitbase.refs, gitbase.commit_parents, gitbase.tree_entries, gitbase.files, gitbase.commits, gitbase.repositories, gitbase.blobs, gitbase.uast_cache, gitbase.uast_functions, gitbase.uast_imports",
    )
    .execute(pool)
    .await?;
    Ok(())
}
