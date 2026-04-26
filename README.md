# GitDatabase

GitDatabase é uma solução que indexa repositórios Git e expõe os dados em PostgreSQL para consultas analíticas e de busca. Ele fornece:

- **Sync de metadados Git** (repositórios, refs, commits, arquivos, árvores)
- **Hidratação de blobs** (conteúdo textual dos arquivos)
- **Busca de código** com `search_code(pattern, lang)`
- **(Opcional) UAST** para análises semânticas
- **Acesso via pgwire/psql** para consultas SQL

> Este repositório é um workspace Rust com múltiplos crates (CLI, DB, loader e pgwire).

---

## Estrutura do projeto

- `crates/gitbase-cli` — CLI principal (sync, search-index, hydrate-blobs, uast, serve)
- `crates/gitbase-db` — acesso ao PostgreSQL e migrations
- `crates/gitbase-loader` — ingestão e hidratação de dados Git
- `crates/gitbase-pgwire` — servidor pgwire (porta padrão 5433)
- `migrations/` — schema do banco
- `docker/docker-compose.yml` — Postgres local

---

## Pré-requisitos

- Rust (Cargo)
- Docker + docker-compose
- PostgreSQL client (`psql`)

---

## Subindo o Postgres (Docker)

```bash
cd docker
docker-compose up -d
```

A conexão padrão é:

- Host: `127.0.0.1`
- Porta: `5433`
- Banco: `gitbase`
- Usuário: `gitbase`
- Senha: `gitbase`

---

## Variáveis de ambiente

- `DATABASE_URL=postgres://gitbase:gitbase@127.0.0.1:5433/gitbase`
- `GITBASE_REPO_ROOTS=/caminho/para/repos`

---

## Uso rápido (CLI)

### 1) Sincronizar repositórios

```bash
DATABASE_URL=postgres://gitbase:gitbase@127.0.0.1:5433/gitbase \
  cargo run -p gitbase-cli -- sync --repo-roots /caminho/para/repos
```

### 2) Hidratar conteúdo dos arquivos (blobs)

Isso preenche `gitbase.blobs.content`, necessário para busca de conteúdo e indexação.

```bash
DATABASE_URL=postgres://gitbase:gitbase@127.0.0.1:5433/gitbase \
  cargo run -p gitbase-cli -- hydrate-blobs --repo-roots /caminho/para/repos
```

> Opcional: limitar com `--limit` via `GITBASE_BLOB_HYDRATE_LIMIT`.

### 3) Criar índice de busca

```bash
DATABASE_URL=postgres://gitbase:gitbase@127.0.0.1:5433/gitbase \
  cargo run -p gitbase-cli -- search-index
```

---

## Acesso via psql

```bash
psql "postgres://gitbase:gitbase@127.0.0.1:5433/gitbase"
```

Se o prompt aparecer como `gitbase-#`, use `\r` para limpar o comando pendente.

---

## Consultas úteis

### Esquemas e tabelas

```sql
SELECT schema_name FROM information_schema.schemata ORDER BY schema_name;
```

```sql
SELECT table_schema, table_name
FROM information_schema.tables
WHERE table_type = 'BASE TABLE'
ORDER BY table_schema, table_name;
```

```sql
SELECT table_name
FROM information_schema.tables
WHERE table_schema = 'gitbase'
  AND table_type = 'BASE TABLE'
ORDER BY table_name;
```

---

### Listar arquivos (distintos)

```sql
SELECT DISTINCT
  r.name AS repository,
  f.path,
  f.language,
  f.size,
  f.is_binary
FROM gitbase.files f
JOIN gitbase.repositories r ON r.id = f.repository_id
ORDER BY r.name, f.path
LIMIT 200;
```

---

### Buscar commits por autor

```sql
SELECT
  c.hash,
  c.author_name,
  c.author_email,
  c.committed_at,
  c.message,
  r.name AS repository
FROM gitbase.commits c
JOIN gitbase.repositories r ON r.id = c.repository_id
WHERE c.author_email = 'email@dominio'
ORDER BY c.committed_at DESC
LIMIT 100;
```

---

### Buscar uso de biblioteca (search_code)

```sql
SELECT
  r.name AS repository,
  s.path,
  s.commit_hash,
  s.language,
  s.score
FROM gitbase.search_code('MimeKit') s
JOIN gitbase.repositories r ON r.id = s.repository_id
ORDER BY s.score DESC
LIMIT 200;
```

---

### Ver conteúdo de arquivo (texto + hex na mesma query)

```sql
SELECT DISTINCT ON (f.path)
  f.path,
  encode(b.content, 'hex') AS content_hex,
  convert_from(b.content, 'UTF8') AS content_text
FROM gitbase.files f
JOIN gitbase.blobs b ON b.hash = f.blob_hash
JOIN gitbase.commits c
  ON c.repository_id = f.repository_id
 AND c.hash = f.commit_hash
WHERE f.path = 'backend/src/CertificateManager.Core/Entities/User.cs'
ORDER BY f.path, c.committed_at DESC
LIMIT 1;
```

---

## Troubleshooting

- **`search_code` retornando 0 linhas**
  - Rode `hydrate-blobs` e depois `search-index`.
- **`ERROR: invalid byte sequence for encoding "UTF8": 0x00`**
  - O índice ignora NUL bytes e normaliza o conteúdo automaticamente.
- **Repetição de arquivos**
  - `gitbase.files` guarda arquivos por commit. Use `DISTINCT` se quiser um por caminho.

---

## Licença

Consulte o arquivo de licença do projeto (se aplicável).
