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

## Relacionamentos lógicos

```text
repositories
├── refs
├── commits
│   ├── commit_parents
│   ├── tree_entries
│   └── files ───────────────┐
└────────────────────────────┤
                             ▼
                           blobs
                             ├── code_index
                             └── uast_cache
                                 ├── uast_functions
                                 └── uast_imports
```

Esses relacionamentos são aplicados pelos joins e chaves primárias, mas as
migrations atuais não definem foreign keys. Assim, o banco não impede órfãos se
um operador executar DML manual ou se um pipeline for interrompido.

## Granularidade e identidade

| Entidade | Chave | Granularidade |
| --- | --- | --- |
| repository | `id` | path descoberto convertido em SHA-1 |
| ref | `repository_id, name` | ref local/remota/tag por repositório |
| commit | `repository_id, hash` | commit alcançável por alguma ref |
| file | `repository_id, commit_hash, path` | ocorrência do path em um commit |
| blob | `hash` | conteúdo Git global e deduplicado |
| code index | `blob_hash` | documento textual por conteúdo |
| UAST | `blob_hash` | documento semântico por conteúdo |

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

Como um blob pode ocorrer em muitos commits, uma função ou import pode aparecer
repetidamente nas views. Filtre por ref/commit ou use `DISTINCT` conforme o caso.

## Ciclo de vida dos dados

| Evento | Efeito atual |
| --- | --- |
| commit novo | novas linhas de commit, tree e file |
| ref movida | target da ref é atualizado |
| ref apagada | linha antiga permanece |
| force-push | novos commits entram; antigos permanecem |
| repo movido | novo ID baseado no path; registro anterior permanece |
| blob repetido | conteúdo e índices são reutilizados pelo hash |
| parser alterado | cache UAST não é invalidado automaticamente |

## Consistência a verificar

```sql
-- Commits sem arquivos e sem tree entries podem indicar carga parcial.
SELECT c.repository_id, c.hash
FROM gitbase.commits c
LEFT JOIN gitbase.files f
  ON f.repository_id = c.repository_id AND f.commit_hash = c.hash
LEFT JOIN gitbase.tree_entries t
  ON t.repository_id = c.repository_id AND t.commit_hash = c.hash
GROUP BY c.repository_id, c.hash
HAVING COUNT(f.path) = 0 AND COUNT(t.path) = 0;
```

```sql
-- Arquivos cujo repositório não existe.
SELECT COUNT(*) AS orphan_files
FROM gitbase.files f
LEFT JOIN gitbase.repositories r ON r.id = f.repository_id
WHERE r.id IS NULL;
```

```sql
-- Cobertura do conteúdo por hash distinto, sem multiplicação por commit.
SELECT
  COUNT(DISTINCT f.blob_hash) AS referenced,
  COUNT(DISTINCT f.blob_hash) FILTER (WHERE b.content IS NOT NULL) AS hydrated
FROM gitbase.files f
LEFT JOIN gitbase.blobs b ON b.hash = f.blob_hash;
```

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
- o filtro `lang` usa `code_index.language`; atualmente a inferência automática
  cobre apenas extensões Go e Rust quando `files.language` está vazio
- não remova rows de `repositories` isoladamente sem avaliar projeções órfãs
- alterações no algoritmo de busca ou parser exigem rebuild manual dos caches

## Evolução planejada

O [plano de funcionalidades](./roadmap.md) propõe foreign keys, reconciliação,
garbage collection, versionamento de cache e uma API de busca orientada a
snapshot.
