-- UAST cache and projections.
CREATE TABLE IF NOT EXISTS gitbase.uast_cache (
  blob_hash char(40) PRIMARY KEY,
  language text NOT NULL,
  uast jsonb NOT NULL,
  generated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS gitbase.uast_functions (
  blob_hash char(40) NOT NULL,
  name text NOT NULL,
  start_line integer,
  end_line integer,
  signature text,
  PRIMARY KEY (blob_hash, name, start_line)
);

CREATE TABLE IF NOT EXISTS gitbase.uast_imports (
  blob_hash char(40) NOT NULL,
  source text NOT NULL,
  target text,
  PRIMARY KEY (blob_hash, source, target)
);

CREATE INDEX IF NOT EXISTS uast_cache_language_idx
  ON gitbase.uast_cache (language);

CREATE INDEX IF NOT EXISTS uast_cache_jsonb_idx
  ON gitbase.uast_cache USING gin (uast);

CREATE OR REPLACE VIEW gitbase.functions AS
  SELECT f.repository_id,
         f.commit_hash,
         f.path,
         uf.blob_hash,
         uf.name,
         uf.start_line,
         uf.end_line,
         uf.signature
  FROM gitbase.uast_functions uf
  JOIN gitbase.files f ON f.blob_hash = uf.blob_hash;

CREATE OR REPLACE VIEW gitbase.imports AS
  SELECT f.repository_id,
         f.commit_hash,
         f.path,
         ui.blob_hash,
         ui.source,
         ui.target
  FROM gitbase.uast_imports ui
  JOIN gitbase.files f ON f.blob_hash = ui.blob_hash;
