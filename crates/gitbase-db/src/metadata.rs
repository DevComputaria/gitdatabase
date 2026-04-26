use anyhow::Result;
use sqlx::{PgPool, Row};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct RepositoryRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub default_ref: Option<String>,
    pub is_bare: bool,
}

#[derive(Debug, Clone)]
pub struct RefRecord {
    pub repository_id: String,
    pub name: String,
    pub target_hash: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct CommitRecord {
    pub repository_id: String,
    pub hash: String,
    pub tree_hash: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub committer_name: Option<String>,
    pub committer_email: Option<String>,
    pub message: Option<String>,
    pub committed_at_seconds: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CommitParentRecord {
    pub repository_id: String,
    pub commit_hash: String,
    pub parent_hash: String,
    pub parent_index: i32,
}

#[derive(Debug, Clone)]
pub struct TreeEntryRecord {
    pub repository_id: String,
    pub commit_hash: String,
    pub path: String,
    pub object_hash: String,
    pub object_type: String,
    pub file_mode: String,
    pub size: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FileRecord {
    pub repository_id: String,
    pub commit_hash: String,
    pub path: String,
    pub blob_hash: String,
    pub language: Option<String>,
    pub size: Option<i64>,
    pub is_binary: bool,
}

pub async fn upsert_repository(pool: &PgPool, record: &RepositoryRecord) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.repositories (id, name, path, default_ref, is_bare)\
         VALUES ($1, $2, $3, $4, $5)\
         ON CONFLICT (id) DO UPDATE \
         SET name = EXCLUDED.name,\
             path = EXCLUDED.path,\
             default_ref = EXCLUDED.default_ref,\
             is_bare = EXCLUDED.is_bare",
    )
    .bind(&record.id)
    .bind(&record.name)
    .bind(&record.path)
    .bind(&record.default_ref)
    .bind(record.is_bare)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn upsert_ref(pool: &PgPool, record: &RefRecord) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.refs (repository_id, name, target_hash, kind)\
         VALUES ($1, $2, $3, $4)\
         ON CONFLICT (repository_id, name) DO UPDATE \
         SET target_hash = EXCLUDED.target_hash,\
             kind = EXCLUDED.kind",
    )
    .bind(&record.repository_id)
    .bind(&record.name)
    .bind(&record.target_hash)
    .bind(&record.kind)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn upsert_commit(pool: &PgPool, record: &CommitRecord) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.commits (repository_id, hash, tree_hash, author_name, author_email,\
         committer_name, committer_email, message, committed_at)\
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8,\
         CASE WHEN $9 IS NULL THEN NULL ELSE to_timestamp($9) END)\
         ON CONFLICT (repository_id, hash) DO UPDATE \
         SET tree_hash = EXCLUDED.tree_hash,\
             author_name = EXCLUDED.author_name,\
             author_email = EXCLUDED.author_email,\
             committer_name = EXCLUDED.committer_name,\
             committer_email = EXCLUDED.committer_email,\
             message = EXCLUDED.message,\
             committed_at = EXCLUDED.committed_at",
    )
    .bind(&record.repository_id)
    .bind(&record.hash)
    .bind(&record.tree_hash)
    .bind(&record.author_name)
    .bind(&record.author_email)
    .bind(&record.committer_name)
    .bind(&record.committer_email)
    .bind(&record.message)
    .bind(record.committed_at_seconds)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn upsert_commit_parent(pool: &PgPool, record: &CommitParentRecord) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.commit_parents (repository_id, commit_hash, parent_hash, parent_index)\
         VALUES ($1, $2, $3, $4)\
         ON CONFLICT (repository_id, commit_hash, parent_index) DO UPDATE \
         SET parent_hash = EXCLUDED.parent_hash",
    )
    .bind(&record.repository_id)
    .bind(&record.commit_hash)
    .bind(&record.parent_hash)
    .bind(record.parent_index)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn upsert_tree_entry(pool: &PgPool, record: &TreeEntryRecord) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.tree_entries (repository_id, commit_hash, path, object_hash,\
         object_type, file_mode, size)\
         VALUES ($1, $2, $3, $4, $5, $6, $7)\
         ON CONFLICT (repository_id, commit_hash, path) DO UPDATE \
         SET object_hash = EXCLUDED.object_hash,\
             object_type = EXCLUDED.object_type,\
             file_mode = EXCLUDED.file_mode,\
             size = EXCLUDED.size",
    )
    .bind(&record.repository_id)
    .bind(&record.commit_hash)
    .bind(&record.path)
    .bind(&record.object_hash)
    .bind(&record.object_type)
    .bind(&record.file_mode)
    .bind(record.size)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn upsert_file(pool: &PgPool, record: &FileRecord) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.files (repository_id, commit_hash, path, blob_hash, language, size, is_binary)\
         VALUES ($1, $2, $3, $4, $5, $6, $7)\
         ON CONFLICT (repository_id, commit_hash, path) DO UPDATE \
         SET blob_hash = EXCLUDED.blob_hash,\
             language = EXCLUDED.language,\
             size = EXCLUDED.size,\
             is_binary = EXCLUDED.is_binary",
    )
    .bind(&record.repository_id)
    .bind(&record.commit_hash)
    .bind(&record.path)
    .bind(&record.blob_hash)
    .bind(&record.language)
    .bind(record.size)
    .bind(record.is_binary)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn fetch_existing_commit_hashes(
    pool: &PgPool,
    repository_id: &str,
) -> Result<HashSet<String>> {
    let rows = sqlx::query("SELECT hash FROM gitbase.commits WHERE repository_id = $1")
        .bind(repository_id)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("hash"))
        .collect())
}
