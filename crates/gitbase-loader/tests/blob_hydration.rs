use std::fs;

use anyhow::Result;
use git2::{Repository, Signature};
use gitbase_db::connect;
use gitbase_loader::{hydrate_blobs, sync_repositories, BlobHydrationConfig};
use sqlx::PgPool;
use tempfile::TempDir;

#[tokio::test]
async fn hydrate_blob_caches_content() -> Result<()> {
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

    let file_path = repo_path.join("README.md");
    fs::write(&file_path, "hello")?;

    let mut index = repo.index()?;
    index.add_path(std::path::Path::new("README.md"))?;
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

    let blob_id = repo.find_blob(tree.get_name("README.md").unwrap().id())?;
    let blob_hash = blob_id.id().to_string();

    let pool = connect(&database_url, 5).await?;
    clear_tables(&pool).await?;

    let _ = sync_repositories(&pool, &[repo_path.clone()]).await?;

    let report = hydrate_blobs(
        &pool,
        &[repo_path.clone()],
        &[blob_hash.clone()],
        &BlobHydrationConfig {
            max_blob_bytes: 1_000_000,
        },
    )
    .await?;

    assert_eq!(report.hydrated, 1);

    let cached = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gitbase.blobs WHERE hash = $1 AND content IS NOT NULL",
    )
    .bind(&blob_hash)
    .fetch_one(&pool)
    .await?;

    assert_eq!(cached, 1);

    Ok(())
}

async fn clear_tables(pool: &PgPool) -> Result<()> {
    sqlx::query("TRUNCATE gitbase.refs, gitbase.commit_parents, gitbase.tree_entries, gitbase.files, gitbase.commits, gitbase.repositories, gitbase.blobs")
        .execute(pool)
        .await?;
    Ok(())
}
