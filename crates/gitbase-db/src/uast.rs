use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct UastCandidate {
    pub blob_hash: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct UastFunctionRecord {
    pub blob_hash: String,
    pub name: String,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UastImportRecord {
    pub blob_hash: String,
    pub source: String,
    pub target: Option<String>,
}

pub async fn uast_cache_exists(pool: &PgPool, blob_hash: &str) -> Result<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM gitbase.uast_cache WHERE blob_hash = $1)",
    )
    .bind(blob_hash)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

pub async fn fetch_uast_candidates(pool: &PgPool, limit: Option<i64>) -> Result<Vec<UastCandidate>> {
    let base_query =
        "SELECT DISTINCT f.blob_hash, f.path FROM gitbase.files f\
         JOIN gitbase.blobs b ON b.hash = f.blob_hash\
         WHERE b.content IS NOT NULL";

    let rows = if let Some(limit) = limit {
        sqlx::query(&format!("{base_query} LIMIT $1"))
            .bind(limit)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query(base_query).fetch_all(pool).await?
    };

    Ok(rows
        .into_iter()
        .map(|row| UastCandidate {
            blob_hash: row.get("blob_hash"),
            path: row.get("path"),
        })
        .collect())
}

pub async fn upsert_uast_cache(
    pool: &PgPool,
    blob_hash: &str,
    language: &str,
    uast: &Value,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.uast_cache (blob_hash, language, uast, generated_at)\
         VALUES ($1, $2, $3, now())\
         ON CONFLICT (blob_hash) DO UPDATE\
         SET language = EXCLUDED.language,\
             uast = EXCLUDED.uast,\
             generated_at = now()",
    )
    .bind(blob_hash)
    .bind(language)
    .bind(uast)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn clear_uast_projections(pool: &PgPool, blob_hash: &str) -> Result<()> {
    sqlx::query("DELETE FROM gitbase.uast_functions WHERE blob_hash = $1")
        .bind(blob_hash)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM gitbase.uast_imports WHERE blob_hash = $1")
        .bind(blob_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_uast_function(pool: &PgPool, record: &UastFunctionRecord) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.uast_functions (blob_hash, name, start_line, end_line, signature)\
         VALUES ($1, $2, $3, $4, $5)\
         ON CONFLICT (blob_hash, name, start_line) DO UPDATE\
         SET end_line = EXCLUDED.end_line,\
             signature = EXCLUDED.signature",
    )
    .bind(&record.blob_hash)
    .bind(&record.name)
    .bind(record.start_line)
    .bind(record.end_line)
    .bind(&record.signature)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_uast_import(pool: &PgPool, record: &UastImportRecord) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.uast_imports (blob_hash, source, target)\
         VALUES ($1, $2, $3)\
         ON CONFLICT (blob_hash, source, target) DO NOTHING",
    )
    .bind(&record.blob_hash)
    .bind(&record.source)
    .bind(&record.target)
    .execute(pool)
    .await?;
    Ok(())
}
