# Arquitetura

Esta página descreve a arquitetura implementada no código atual do GitDatabase.

## Como ler esta documentação

- [Contexto e componentes](./architecture/system-context.md): fronteiras,
  processos, crates, topologias e segurança
- [Pipelines e consistência](./architecture/data-pipelines.md): ciclo de vida dos
  dados, idempotência e recuperação
- [Atributos de qualidade](./architecture/quality-attributes.md): riscos de
  produção, capacidade, segurança e estratégia de testes
- [Banco de dados](./database.md): modelo relacional, índices e consultas
- [Plano de funcionalidades](./roadmap.md): lacunas priorizadas e critérios de
  aceite

## Princípios implementados

1. **PostgreSQL como plataforma de dados:** persistência, busca e consultas usam
   capacidades nativas do banco.
2. **Conteúdo endereçado por hash:** blobs, índice textual e UAST são
   deduplicados por SHA-1 do objeto Git.
3. **Ingestão em estágios:** metadata, conteúdo, busca e semântica podem evoluir
   e ser reexecutados separadamente.
4. **Git permanece fonte de verdade:** o banco é uma projeção consultável; a
   hidratação ainda depende dos objetos locais.
5. **Limites antes de parsing:** tamanho, binário e UTF-8 protegem os estágios de
   conteúdo.

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

O pipeline é eventualmente consistente: concluir `sync` não implica que blobs,
busca ou UAST estejam atualizados. Consulte a
[matriz de reexecução](./architecture/data-pipelines.md#matriz-de-reexecucao).

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

## Invariantes e limites

- `repositories.path` é único e também participa indiretamente da identidade do
  repositório.
- commits, files e tree entries pertencem a um repositório lógico, mas essa
  relação ainda não é protegida por foreign keys.
- `files` representa ocorrências por commit; não é uma visão apenas do HEAD.
- `blobs`, `code_index` e `uast_cache` são globais por hash.
- uma linha em `blobs` com `content IS NULL` pode representar binário, objeto
  acima do limite ou hidratação ainda pendente.
- a implementação atual não reconcilia deleções e force-pushes.
- o listener pgwire usa autenticação cleartext e deve permanecer em rede
  protegida.

## Decisões que ainda precisam de ADR

- identidade estável do repositório: path, remote URL ou UUID;
- retenção de histórico após refs deixarem de alcançar commits;
- atomicidade por commit versus por repositório;
- versionamento e invalidação dos caches search/UAST;
- exposição do pgwire como gateway administrativo ou somente leitura.

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

## Estado de maturidade

O projeto implementa um pipeline funcional e possui testes unitários, integração
com PostgreSQL e um cenário E2E em containers. Antes de produção multiusuário,
as prioridades são consistência transacional, reconciliação, integridade
referencial, alinhamento do toolchain e endurecimento do pgwire. A ordem proposta
está no [roadmap](./roadmap.md).
