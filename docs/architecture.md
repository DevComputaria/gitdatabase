# Arquitetura

Esta página descreve a arquitetura implementada no código atual do GitDatabase.

## Workspace e crates

O projeto é um workspace Rust com os seguintes crates:

- `gitbase-cli`: interface de linha de comando e orquestração dos fluxos
- `gitbase-db`: conexão com PostgreSQL, migrações e acesso a dados
- `gitbase-git`: leitura de repositórios Git com `gix`
- `gitbase-loader`: pipeline de ingestão/indexação
- `gitbase-uast`: parsing semântico (Go/Rust) via tree-sitter
- `gitbase-pgwire`: servidor compatível com protocolo PostgreSQL

## Comandos da CLI

Comandos disponíveis em `gitbase-cli`:

- `serve`: sobe servidor pgwire
- `sync`: sincroniza metadados Git
- `health`: checa conectividade e migrações
- `hydrate-blobs`: hidrata conteúdo textual dos blobs
- `search-index`: monta/atualiza índice de busca textual
- `uast`: gera cache/projeções de UAST

## Fluxo de dados

```text
Repositórios Git
   ↓ (gitbase-git)
Metadados (repos/refs/commits/tree/files)
   ↓ (gitbase-loader + gitbase-db)
PostgreSQL (schema gitbase)
   ↓
Blobs hidratados + índice de busca + UAST
   ↓
Consultas SQL / pgwire
```

## Detalhes de implementação relevantes

### 1) Descoberta e leitura Git (`gitbase-git`)

- Usa `gix` para abrir repositórios e caminhar histórico
- Suporta roots com `.git/` e repositórios bare
- Gera snapshots de commit contendo:
  - metadados do commit
  - entradas de árvore
  - arquivos por commit
- Classifica blobs binários por NUL byte e validação UTF-8

### 2) Sincronização (`gitbase-loader::sync_repositories`)

- Faz upsert em:
  - `repositories`
  - `refs`
  - `commits`
  - `commit_parents`
  - `tree_entries`
  - `files`
- Evita retrabalho para commits já persistidos

### 3) Hidratação de blobs (`hydrate_blobs` / `hydrate_missing_blobs`)

- Busca blobs faltantes no banco
- Lê blob no Git de origem
- Regras:
  - blob maior que `max_blob_bytes` → marca sem conteúdo
  - blob binário → marca sem conteúdo
  - blob textual válido → persiste `content`

### 4) Indexação de busca (`index_search`)

- Candidatos vêm de `files` + `blobs`
- Só indexa conteúdo textual UTF-8
- Normaliza NUL (`\0`) para espaço
- Persiste `tsvector` em `code_index`

### 5) Indexação UAST (`index_uast`)

- Suporta linguagens detectadas por extensão:
  - `.go`
  - `.rs`
- Armazena documento UAST em JSONB (`uast_cache`)
- Projeta funções e imports em tabelas específicas

### 6) Consulta via pgwire (`gitbase-pgwire`)

- Server escuta em `bind` (default `0.0.0.0:5433`)
- Autenticação simples por usuário/senha configuráveis
- Encaminha queries para PostgreSQL real (`sqlx`)
- Em consultas `SELECT`, tenta hidratar blobs referenciados por `blob_hash` antes de executar

## Variáveis de ambiente mais usadas

- `DATABASE_URL`
- `GITBASE_REPO_ROOTS`
- `GITBASE_DB_MAX_CONNECTIONS`
- `GITBASE_BLOB_MAX_BYTES`
- `GITBASE_BLOB_HYDRATE_LIMIT`
- `GITBASE_SEARCH_LIMIT`
- `GITBASE_UAST_LIMIT`
- `GITBASE_BIND_ADDR`
- `GITBASE_PG_USER`
- `GITBASE_PG_PASSWORD`
