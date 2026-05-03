# Banco de dados

Esta página descreve o modelo de dados criado pelas migrations atuais.

## Extensões e schema

- Extensão: `pg_trgm`
- Schema principal: `gitbase`

## Tabelas principais

### Metadados Git

- `gitbase.repositories`
- `gitbase.refs`
- `gitbase.commits`
- `gitbase.commit_parents`
- `gitbase.tree_entries`
- `gitbase.files`

### Cache de blobs

- `gitbase.blobs`

### Busca textual

- `gitbase.code_index`

### UAST

- `gitbase.uast_cache`
- `gitbase.uast_functions`
- `gitbase.uast_imports`

## Índices relevantes

- `files_path_trgm_idx` (GIN trigram em `files.path`)
- `code_index_search_vector_gin` (GIN em `search_vector`)
- `code_index_language_idx`
- índices auxiliares de join por repositório/commit

## Função de busca

A migration cria:

- `gitbase.search_code(pattern TEXT, lang TEXT DEFAULT NULL)`

Retorno:

- `repository_id`
- `commit_hash`
- `path`
- `blob_hash`
- `language`
- `score`

Implementação usa `websearch_to_tsquery('simple', pattern)` e `ts_rank_cd(...)`.

## Views de projeção UAST

- `gitbase.functions`
- `gitbase.imports`

As views fazem join entre projeções UAST e `files` por `blob_hash` para expor contexto de repositório/commit/path.

## Consultas úteis

### 1) Quantidade de repositórios sincronizados

```sql
SELECT COUNT(*) AS repos
FROM gitbase.repositories;
```

### 2) Estado de hidratação de blobs

```sql
SELECT
  COUNT(*) FILTER (WHERE content IS NOT NULL) AS hydrated,
  COUNT(*) FILTER (WHERE content IS NULL) AS without_content,
  COUNT(*) AS total
FROM gitbase.blobs;
```

### 3) Cobertura do índice de busca

```sql
SELECT
  COUNT(*) AS indexed_blobs,
  COUNT(DISTINCT language) AS languages
FROM gitbase.code_index;
```

### 4) Buscar código

```sql
SELECT repository_id, path, language, score
FROM gitbase.search_code('http client', NULL)
LIMIT 20;
```

### 5) Funções parseadas (UAST)

```sql
SELECT repository_id, path, name, start_line, end_line
FROM gitbase.functions
ORDER BY repository_id, path, start_line
LIMIT 100;
```

## Observações operacionais

- `files` é versionada por commit (mesmo path pode aparecer várias vezes)
- `code_index` indexa por `blob_hash` (um blob pode estar em múltiplos commits)
- UAST depende de blob textual hidratado e linguagem suportada
