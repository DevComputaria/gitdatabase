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

## Checklist antes de operar

1. confirme a versão do binário e do schema;
2. valide espaço disponível no PostgreSQL;
3. garanta que os mesmos paths de repositório usados no sync estão montados para
   a hidratação;
4. execute `health`;
5. registre contagens de baseline;
6. evite dois processos de sync sobre o mesmo repositório até existir lock
   distribuído;
7. para manutenção destrutiva, faça backup e registre as tabelas afetadas.

## Teste de integração em containers

```bash
mkdir -p test-results
docker compose -f docker/docker-compose.integration-test.yml up \
  --build \
  --abort-on-container-exit \
  --exit-code-from integration-test \
  integration-test
```

O exit code do serviço `integration-test` define o resultado. O relatório contém
o commit clonado e as contagens observadas. Sempre encerre a infraestrutura:

```bash
docker compose -f docker/docker-compose.integration-test.yml down \
  --volumes --remove-orphans
```

## Reindexação

!!! warning
    Os comandos abaixo removem dados derivados. Faça backup quando o ambiente não
    puder ser reconstruído imediatamente e evite executá-los enquanto indexadores
    ou clientes pgwire estiverem ativos.

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

### Detectar possível sync parcial

```sql
SELECT c.repository_id, c.hash
FROM gitbase.commits c
LEFT JOIN gitbase.files f
  ON f.repository_id = c.repository_id AND f.commit_hash = c.hash
LEFT JOIN gitbase.tree_entries t
  ON t.repository_id = c.repository_id AND t.commit_hash = c.hash
GROUP BY c.repository_id, c.hash
HAVING COUNT(f.path) = 0 AND COUNT(t.path) = 0;
```

### Acompanhar idade dos caches

```sql
SELECT
  MIN(cached_at) AS oldest_cached,
  MAX(cached_at) AS newest_cached,
  MIN(last_used_at) AS oldest_use,
  MAX(last_used_at) AS newest_use
FROM gitbase.blobs;
```

## Backup e restore

O estado persistente está no PostgreSQL. Repositórios Git continuam necessários
para hidratar hashes ausentes.

```bash
pg_dump --format=custom --schema=gitbase "$DATABASE_URL" \
  --file gitbase.dump
```

Restaure primeiro em um banco separado, execute `health` e compare contagens antes
de promover o banco. A tabela `_sqlx_migrations` está fora do schema `gitbase`;
para um backup completo da instância GitDatabase, inclua-a ou use dump do banco
inteiro.

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

### Dados antigos após branch removida ou force-push

- Este é um limite conhecido: o sync atual atualiza refs existentes, mas não
  remove refs ausentes nem commits inalcançáveis.
- Não apague commits manualmente sem considerar files, trees, parents e caches.
- Registre o caso e siga a política de retenção definida para o ambiente; a
  reconciliação automática está planejada em GD-004.

### Commit existe, mas files/tree entries estão incompletos

- Uma repetição comum não reprocessa o snapshot de um commit já cadastrado.
- Restaure de backup ou remova cuidadosamente todas as projeções daquele commit
  dentro de uma transação antes de repetir o sync.
- O modo de reparo está planejado em GD-003.
