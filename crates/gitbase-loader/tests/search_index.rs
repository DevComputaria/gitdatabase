use std::fs;

use anyhow::Result;
use git2::{Repository, Signature};
use gitbase_db::connect;
use gitbase_loader::{
    hydrate_blobs, index_search, sync_repositories, BlobHydrationConfig, SearchIndexConfig,
};
use sqlx::PgPool;
use tempfile::TempDir;

#[tokio::test]
async fn search_index_populates_code_index() -> Result<()> {
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

    let file_path = repo_path.join("main.rs");
    fs::write(&file_path, "fn main() { println!(\"hello\"); }")?;

    let mut index = repo.index()?;
    index.add_path(std::path::Path::new("main.rs"))?;
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

    let head_commit = repo.head()?.peel_to_commit()?;
    let head_tree = head_commit.tree()?;
    let entry = head_tree.get_path(std::path::Path::new("main.rs"))?;
    let blob_hash = entry.id().to_string();

    let pool = connect(&database_url, 5).await?;
    clear_tables(&pool).await?;

    let _report = sync_repositories(&pool, &[repo_path.clone()]).await?;

    let hydration_report = hydrate_blobs(
        &pool,
        &[repo_path.clone()],
        &[blob_hash.clone()],
        &BlobHydrationConfig {
            max_blob_bytes: 1_000_000,
        },
    )
    .await?;
    assert!(hydration_report.hydrated >= 1);

    let search_report = index_search(
        &pool,
        &SearchIndexConfig {
            max_candidates: None,
        },
    )
    .await?;
    assert!(search_report.indexed >= 1);

    let indexed = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gitbase.code_index WHERE blob_hash = $1",
    )
    .bind(&blob_hash)
    .fetch_one(&pool)
    .await?;

    assert_eq!(indexed, 1);

    Ok(())
}

async fn clear_tables(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "TRUNCATE gitbase.code_index, gitbase.blobs, gitbase.refs, gitbase.commit_parents, gitbase.tree_entries, gitbase.files, gitbase.commits, gitbase.repositories",
    )
    .execute(pool)
    .await?;
    Ok(())
}
