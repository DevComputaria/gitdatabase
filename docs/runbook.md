# Runbook

Guia operacional para execução, manutenção e troubleshooting do GitDatabase.

## Operações comuns

### 1) Health check

```bash
cargo run -p gitbase-cli -- health --database-url "$DATABASE_URL"
```

### 2) Sync de metadados

```bash
cargo run -p gitbase-cli -- sync \
	--database-url "$DATABASE_URL" \
	--repo-roots /path/to/repos
```

### 3) Hidratação de blobs

```bash
cargo run -p gitbase-cli -- hydrate-blobs \
	--database-url "$DATABASE_URL" \
	--repo-roots /path/to/repos \
	--blob-max-bytes 1000000 \
	--limit 5000
```

### 4) Índice de busca

```bash
cargo run -p gitbase-cli -- search-index \
	--database-url "$DATABASE_URL" \
	--limit 5000
```

### 5) UAST

```bash
cargo run -p gitbase-cli -- uast \
	--database-url "$DATABASE_URL" \
	--limit 5000
```

### 6) Servidor pgwire

```bash
cargo run -p gitbase-cli -- serve \
	--database-url "$DATABASE_URL" \
	--repo-roots /path/to/repos \
	--bind 0.0.0.0:5433 \
	--pg-user gitbase \
	--pg-password gitbase
```

## Ordem recomendada de processamento

Para carga inicial:

1. `health`
2. `sync`
3. `hydrate-blobs`
4. `search-index`
5. `uast` (se necessário)

Para operação incremental diária:

1. `sync`
2. `hydrate-blobs --limit ...`
3. `search-index --limit ...`
4. `uast --limit ...` (quando aplicável)

## Reindexação

### Recriar índice de busca

```sql
TRUNCATE gitbase.code_index;
```

Em seguida rode `search-index` novamente.

### Recriar cache/projeções UAST

```sql
TRUNCATE gitbase.uast_cache, gitbase.uast_functions, gitbase.uast_imports;
```

Em seguida rode `uast` novamente.

### Rehidratar blobs

Para forçar nova hidratação, limpe o cache de blobs:

```sql
TRUNCATE gitbase.blobs;
```

Depois rode `hydrate-blobs`.

## Diagnóstico rápido

### Verificar cobertura do pipeline

```sql
SELECT
	(SELECT COUNT(*) FROM gitbase.repositories) AS repos,
	(SELECT COUNT(*) FROM gitbase.commits) AS commits,
	(SELECT COUNT(*) FROM gitbase.files) AS files,
	(SELECT COUNT(*) FROM gitbase.blobs WHERE content IS NOT NULL) AS hydrated_blobs,
	(SELECT COUNT(*) FROM gitbase.code_index) AS indexed_blobs,
	(SELECT COUNT(*) FROM gitbase.uast_cache) AS uast_docs;
```

### Conferir arquivos sem blob hidratado

```sql
SELECT COUNT(*) AS files_without_blob_content
FROM gitbase.files f
LEFT JOIN gitbase.blobs b ON b.hash = f.blob_hash
WHERE b.content IS NULL;
```

### Conferir linguagens indexadas

```sql
SELECT language, COUNT(*)
FROM gitbase.code_index
GROUP BY language
ORDER BY COUNT(*) DESC;
```

## Troubleshooting

### `search_code` retorna vazio

- Rode `hydrate-blobs` e depois `search-index`.
- Verifique se há blobs com `content IS NOT NULL`.
- Confira se o `pattern` não está vazio.

### UAST com baixa cobertura

- Atualmente apenas `.go` e `.rs` são suportados.
- Verifique se os blobs foram hidratados com conteúdo UTF-8.

### Sync não encontra repositórios

- Confirme `--repo-roots` / `GITBASE_REPO_ROOTS`.
- Verifique permissões de leitura no filesystem.
- Confirme que os paths contêm `.git/` ou formato bare.

### Degradação de performance

- Reduza escopo com `--limit` nos indexadores.
- Ajuste `--max-connections` e conexão do PostgreSQL.
- Execute cargas pesadas fora do horário de pico.
