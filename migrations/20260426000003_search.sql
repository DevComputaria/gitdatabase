CREATE TABLE IF NOT EXISTS gitbase.code_index (
    blob_hash CHAR(40) PRIMARY KEY,
    language TEXT,
    search_vector tsvector NOT NULL,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS code_index_search_vector_gin
    ON gitbase.code_index USING GIN (search_vector);

CREATE INDEX IF NOT EXISTS code_index_language_idx
    ON gitbase.code_index (language);

CREATE OR REPLACE FUNCTION gitbase.search_code(pattern TEXT, lang TEXT DEFAULT NULL)
RETURNS TABLE (
    repository_id TEXT,
    commit_hash TEXT,
    path TEXT,
    blob_hash TEXT,
    language TEXT,
    score REAL
)
LANGUAGE sql STABLE AS $$
    SELECT
        f.repository_id,
        f.commit_hash,
        f.path,
        ci.blob_hash,
        ci.language,
        ts_rank_cd(ci.search_vector, query.query) AS score
    FROM gitbase.code_index ci
    JOIN gitbase.files f ON f.blob_hash = ci.blob_hash
    CROSS JOIN LATERAL (
        SELECT CASE
            WHEN pattern IS NULL OR btrim(pattern) = '' THEN NULL
            ELSE websearch_to_tsquery('simple', pattern)
        END AS query
    ) AS query
    WHERE query.query IS NOT NULL
      AND query.query @@ ci.search_vector
      AND (lang IS NULL OR ci.language = lang)
    ORDER BY score DESC;
$$;
