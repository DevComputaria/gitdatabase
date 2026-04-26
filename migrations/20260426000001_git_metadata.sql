-- Base schema for Git metadata.
CREATE SCHEMA IF NOT EXISTS gitbase;

CREATE TABLE IF NOT EXISTS gitbase.repositories (
  id text PRIMARY KEY,
  name text NOT NULL,
  path text NOT NULL UNIQUE,
  default_ref text,
  is_bare boolean NOT NULL,
  discovered_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS gitbase.refs (
  repository_id text NOT NULL,
  name text NOT NULL,
  target_hash char(40) NOT NULL,
  kind text NOT NULL,
  PRIMARY KEY (repository_id, name)
);

CREATE TABLE IF NOT EXISTS gitbase.commits (
  repository_id text NOT NULL,
  hash char(40) NOT NULL,
  tree_hash char(40) NOT NULL,
  author_name text,
  author_email text,
  committer_name text,
  committer_email text,
  message text,
  committed_at timestamptz,
  PRIMARY KEY (repository_id, hash)
);

CREATE TABLE IF NOT EXISTS gitbase.commit_parents (
  repository_id text NOT NULL,
  commit_hash char(40) NOT NULL,
  parent_hash char(40) NOT NULL,
  parent_index integer NOT NULL,
  PRIMARY KEY (repository_id, commit_hash, parent_index)
);

CREATE TABLE IF NOT EXISTS gitbase.tree_entries (
  repository_id text NOT NULL,
  commit_hash char(40) NOT NULL,
  path text NOT NULL,
  object_hash char(40) NOT NULL,
  object_type text NOT NULL,
  file_mode text NOT NULL,
  size bigint,
  PRIMARY KEY (repository_id, commit_hash, path)
);

CREATE TABLE IF NOT EXISTS gitbase.files (
  repository_id text NOT NULL,
  commit_hash char(40) NOT NULL,
  path text NOT NULL,
  blob_hash char(40) NOT NULL,
  language text,
  size bigint,
  is_binary boolean NOT NULL,
  PRIMARY KEY (repository_id, commit_hash, path)
);

CREATE TABLE IF NOT EXISTS gitbase.blobs (
  hash char(40) PRIMARY KEY,
  size bigint NOT NULL,
  is_binary boolean NOT NULL,
  content bytea,
  cached_at timestamptz,
  last_used_at timestamptz
);

CREATE INDEX IF NOT EXISTS tree_entries_repo_commit_idx
  ON gitbase.tree_entries (repository_id, commit_hash);

CREATE INDEX IF NOT EXISTS files_repo_commit_idx
  ON gitbase.files (repository_id, commit_hash);

CREATE INDEX IF NOT EXISTS files_path_trgm_idx
  ON gitbase.files USING gin (path gin_trgm_ops);
