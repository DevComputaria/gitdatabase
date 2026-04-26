use anyhow::Result;
use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct SearchCandidate {
    pub blob_hash: String,
    pub path: String,
    pub language: Option<String>,
}

pub async fn fetch_search_candidates(
    pool: &PgPool,
    max_candidates: Option<i64>,
) -> Result<Vec<SearchCandidate>> {
    let rows = if let Some(limit) = max_candidates {
        sqlx::query(
                        "SELECT DISTINCT ON (f.blob_hash) f.blob_hash, f.path, f.language \
                         FROM gitbase.files f \
                         JOIN gitbase.blobs b ON b.hash = f.blob_hash \
                         LEFT JOIN gitbase.code_index ci ON ci.blob_hash = f.blob_hash \
                         WHERE b.is_binary = false \
                             AND b.content IS NOT NULL \
                             AND ci.blob_hash IS NULL \
                         ORDER BY f.blob_hash, f.path \
                         LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
                        "SELECT DISTINCT ON (f.blob_hash) f.blob_hash, f.path, f.language \
                         FROM gitbase.files f \
                         JOIN gitbase.blobs b ON b.hash = f.blob_hash \
                         LEFT JOIN gitbase.code_index ci ON ci.blob_hash = f.blob_hash \
                         WHERE b.is_binary = false \
                             AND b.content IS NOT NULL \
                             AND ci.blob_hash IS NULL \
                         ORDER BY f.blob_hash, f.path",
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(|row| SearchCandidate {
            blob_hash: row.get("blob_hash"),
            path: row.get("path"),
            language: row.get("language"),
        })
        .collect())
}

pub async fn upsert_code_index(
    pool: &PgPool,
    blob_hash: &str,
    language: Option<&str>,
    content: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO gitbase.code_index (blob_hash, language, search_vector, indexed_at)\
         VALUES ($1, $2, to_tsvector('simple', $3), now())\
         ON CONFLICT (blob_hash) DO UPDATE \
         SET language = EXCLUDED.language,\
             search_vector = EXCLUDED.search_vector,\
             indexed_at = now()",
    )
    .bind(blob_hash)
    .bind(language)
    .bind(content)
    .execute(pool)
    .await?;

    Ok(())
}
