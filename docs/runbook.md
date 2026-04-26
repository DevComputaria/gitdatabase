# Gitbase v2 Runbook

## Common operations

### Sync repositories

```
./target/release/gitbase sync --repo-roots /path/to/repos --database-url "$DATABASE_URL"
```

### Build search index

```
./target/release/gitbase search-index --database-url "$DATABASE_URL" --limit 5000
```

### Build UAST cache

```
./target/release/gitbase uast --database-url "$DATABASE_URL" --limit 5000
```

## Reindexing

- Clear search index:

```
TRUNCATE gitbase.code_index;
```

- Clear UAST cache:

```
TRUNCATE gitbase.uast_cache, gitbase.uast_functions, gitbase.uast_imports;
```

## Troubleshooting

- If search results are empty, ensure blobs are hydrated (the search index only covers cached content).
- If sync skips too much, confirm the repository IDs are stable and the target repo path is correct.
- For large repositories, use limits during pilot runs to control runtime.
