# Gitbase v2 Pilot Guide

This guide describes the pilot workflow for loading repositories, indexing code search, and validating the results.

## Prerequisites

- PostgreSQL 16 with `pg_trgm` enabled
- Rust toolchain 1.80+ or the provided runtime Docker image
- A directory with Git repositories to index

## Quick start (local)

1. Configure environment variables (copy `.env.example` to `.env` and adjust).
2. Start PostgreSQL and apply migrations by running any gitbase command (they auto-apply).
3. Sync repositories:

```
./target/release/gitbase sync --repo-roots /path/to/repos --database-url "$DATABASE_URL"
```

4. (Optional) Hydrate blobs by running a query through the pgwire server or using your workload.
5. Build search index:

```
./target/release/gitbase search-index --database-url "$DATABASE_URL"
```

6. Build UAST cache and projections:

```
./target/release/gitbase uast --database-url "$DATABASE_URL"
```

7. Run a search query:

```
SELECT *
FROM gitbase.search_code('http client', NULL)
LIMIT 20;
```

## Notes

- Incremental sync skips commits already present in the catalog to avoid reprocessing.
- Search index only includes non-binary blobs that have been hydrated.
- Use `GITBASE_SEARCH_LIMIT` and `GITBASE_UAST_LIMIT` to cap indexing work during the pilot.
