use std::fs;

use anyhow::Result;
use gitbase_db::connect;
use gitbase_loader::sync_repositories;
use git2::{IndexAddOption, Repository, Signature};
use sqlx::PgPool;
use tempfile::TempDir;

#[tokio::test]
async fn sync_ingests_basic_metadata() -> Result<()> {
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

    let pool = connect(&database_url, 5).await?;
    clear_tables(&pool).await?;

    let report = sync_repositories(&pool, &[repo_path.clone()]).await?;

    assert!(report.repositories >= 1);
    assert!(report.refs >= 1);
    assert!(report.commits >= 1);
    assert!(report.tree_entries >= 1);
    assert!(report.files >= 1);

    Ok(())
}

async fn clear_tables(pool: &PgPool) -> Result<()> {
    sqlx::query("TRUNCATE gitbase.refs, gitbase.commit_parents, gitbase.tree_entries, gitbase.files, gitbase.commits, gitbase.repositories")
        .execute(pool)
        .await?;
    Ok(())
}
