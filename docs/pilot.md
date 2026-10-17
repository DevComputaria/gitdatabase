# Pilot Guide

Este guia mostra um fluxo de validação ponta a ponta do GitDatabase com os comandos existentes no código atual.

## Pré-requisitos

- PostgreSQL 16+
- Rust 1.89 para o `Cargo.lock` atual
- Repositórios Git para indexar

> O `pg_trgm` é habilitado automaticamente pelas migrations executadas na conexão.

## Variáveis de ambiente

Exemplo:

```bash
export DATABASE_URL="postgres://gitbase:gitbase@127.0.0.1:5433/gitbase"
export GITBASE_REPO_ROOTS="/path/to/repos"
```

## Fluxo recomendado

### 1) Health check + migrations

```bash
cargo run -p gitbase-cli -- health --database-url "$DATABASE_URL"
```

### 2) Sincronizar metadados Git

```bash
cargo run -p gitbase-cli -- sync \
	--database-url "$DATABASE_URL" \
	--repo-roots /path/to/repos
```

### 3) Hidratar blobs faltantes

```bash
cargo run -p gitbase-cli -- hydrate-blobs \
	--database-url "$DATABASE_URL" \
	--repo-roots /path/to/repos \
	--blob-max-bytes 1000000
```

### 4) Montar índice de busca

```bash
cargo run -p gitbase-cli -- search-index \
	--database-url "$DATABASE_URL"
```

### 5) Montar UAST (opcional)

```bash
cargo run -p gitbase-cli -- uast \
	--database-url "$DATABASE_URL"
```

### 6) Validar busca SQL

```sql
SELECT repository_id, path, language, score
FROM gitbase.search_code('http client', NULL)
LIMIT 20;
```

## Validação por contagem

```sql
SELECT COUNT(*) FROM gitbase.repositories;
SELECT COUNT(*) FROM gitbase.commits;
SELECT COUNT(*) FROM gitbase.files;
SELECT COUNT(*) FROM gitbase.blobs WHERE content IS NOT NULL;
SELECT COUNT(*) FROM gitbase.code_index;
```

## Validação automatizada em containers

O cenário mantido pela CI clona `DevComputaria/DevComputaria.Prompting`, executa
o pipeline e aplica asserções SQL:

```bash
mkdir -p test-results
docker compose -f docker/docker-compose.integration-test.yml up \
  --build \
  --abort-on-container-exit \
  --exit-code-from integration-test \
  integration-test
```

O relatório fica em `test-results/container-integration-test.md`. Finalize com:

```bash
docker compose -f docker/docker-compose.integration-test.yml down \
  --volumes --remove-orphans
```

## Observações importantes

- `sync` é incremental para commits já conhecidos.
- `search-index` só indexa blobs com conteúdo textual UTF-8.
- `uast` atualmente cobre arquivos Go (`.go`) e Rust (`.rs`).
- Use limites no piloto para controlar carga:
	- `--limit` em `search-index`
	- `--limit` em `uast`
	- `--limit` em `hydrate-blobs`
- O requisito Rust 1.89 reflete as dependências travadas atuais, apesar de o
  manifest e o Dockerfile principal ainda declararem 1.80; o alinhamento está
  planejado como GD-001.
