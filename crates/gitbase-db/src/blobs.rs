use anyhow::Result;
use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct BlobCache {
    pub hash: String,
    pub size: i64,
    pub is_binary: bool,
    pub has_content: bool,
}

#[derive(Debug, Clone)]
pub struct BlobLocation {
    pub blob_hash: String,
    pub repository_id: String,
    pub repository_path: String,
}

pub async fn fetch_blob_cache(pool: &PgPool, hash: &str) -> Result<Option<BlobCache>> {
    let row = sqlx::query(
        r#"SELECT hash, size, is_binary, (content IS NOT NULL) AS has_content
FROM gitbase.blobs WHERE hash = $1"#,
    )
    .bind(hash)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| BlobCache {
        hash: row.get("hash"),
        size: row.get::<i64, _>("size"),
        is_binary: row.get::<bool, _>("is_binary"),
        has_content: row.get::<bool, _>("has_content"),
    }))
}

pub async fn touch_blob(pool: &PgPool, hash: &str) -> Result<()> {
    sqlx::query("UPDATE gitbase.blobs SET last_used_at = now() WHERE hash = $1")
        .bind(hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn upsert_blob(
    pool: &PgPool,
    hash: &str,
    size: i64,
    is_binary: bool,
    content: Option<&[u8]>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO gitbase.blobs (hash, size, is_binary, content, cached_at, last_used_at)
VALUES ($1, $2, $3, $4, CASE WHEN $4 IS NULL THEN NULL ELSE now() END, now())
ON CONFLICT (hash) DO UPDATE
SET size = EXCLUDED.size,
    is_binary = EXCLUDED.is_binary,
    content = CASE WHEN EXCLUDED.is_binary THEN NULL ELSE COALESCE(EXCLUDED.content, gitbase.blobs.content) END,
    cached_at = CASE
        WHEN EXCLUDED.is_binary THEN NULL
        WHEN EXCLUDED.content IS NOT NULL THEN now()
        ELSE gitbase.blobs.cached_at
    END,
    last_used_at = now()"#,
    )
    .bind(hash)
    .bind(size)
    .bind(is_binary)
    .bind(content)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn fetch_blob_locations(
    pool: &PgPool,
    blob_hashes: &[String],
) -> Result<Vec<BlobLocation>> {
    if blob_hashes.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"SELECT DISTINCT f.blob_hash, r.id AS repository_id, r.path AS repository_path
FROM gitbase.files f
JOIN gitbase.repositories r ON r.id = f.repository_id
WHERE f.blob_hash = ANY($1)"#,
    )
    .bind(blob_hashes)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| BlobLocation {
            blob_hash: row.get("blob_hash"),
            repository_id: row.get("repository_id"),
            repository_path: row.get("repository_path"),
        })
        .collect())
}

pub async fn fetch_blob_content(pool: &PgPool, hash: &str) -> Result<Option<Vec<u8>>> {
    let row = sqlx::query("SELECT content FROM gitbase.blobs WHERE hash = $1")
        .bind(hash)
        .fetch_optional(pool)
        .await?;

    Ok(row.and_then(|row| row.get::<Option<Vec<u8>>, _>("content")))
}

pub async fn fetch_missing_blob_hashes(pool: &PgPool, limit: Option<i64>) -> Result<Vec<String>> {
    let rows = if let Some(limit) = limit {
        sqlx::query(
            r#"SELECT DISTINCT f.blob_hash
FROM gitbase.files f
LEFT JOIN gitbase.blobs b ON b.hash = f.blob_hash
WHERE f.is_binary = false
    AND b.content IS NULL
ORDER BY f.blob_hash
LIMIT $1"#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"SELECT DISTINCT f.blob_hash
FROM gitbase.files f
LEFT JOIN gitbase.blobs b ON b.hash = f.blob_hash
WHERE f.is_binary = false
    AND b.content IS NULL
ORDER BY f.blob_hash"#,
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("blob_hash"))
        .collect())
}
