# Changelog

Este arquivo registra mudanças relevantes do GitDatabase a partir do histórico
Git. O projeto ainda não possui tags de release; por isso, as seções históricas
usam data e marco funcional em vez de atribuir versões que não foram publicadas.

O formato segue as categorias do [Keep a Changelog](https://keepachangelog.com/),
adaptadas para manter rastreabilidade entre commits e arquivos.

## Unreleased

### Added

- Documentação de contexto, componentes e topologias do sistema.
- Documentação detalhada dos pipelines, garantias de idempotência, falhas e
  recuperação.
- Matriz de atributos de qualidade, riscos e critérios de prontidão para
  produção.
- Roadmap priorizado com onze funcionalidades ausentes, dependências e critérios
  de aceite.
- GitHub Action separada para clonar um repositório real, indexar metadados e
  conteúdo, executar consultas SQL e publicar relatório no Job Summary.
- Docker Compose e imagem de teste com PostgreSQL efêmero, GitDatabase, `git` e
  `psql`.

### Changed

- A documentação principal agora explicita princípios, invariantes, limitações,
  ciclo de vida dos dados, integridade, backup e troubleshooting.
- A navegação do MkDocs agrupa as novas visões de arquitetura e inclui o roadmap.
- O piloto documenta Rust 1.89 como requisito efetivo do `Cargo.lock` atual e
  inclui o cenário E2E em containers.
- Saídas locais de MkDocs e do teste em containers são ignoradas pelo Git.

### Validation

- `mkdocs build --strict` concluído com sucesso.
- Teste E2E em containers concluído com 9/9 asserções SQL aprovadas sobre
  `DevComputaria/DevComputaria.Prompting`.

### Traceability

| Commit | Mudança | Arquivos principais |
| --- | --- | --- |
| `2306dc5` | arquitetura detalhada e roadmap | `docs/architecture/**`, `docs/roadmap.md`, `docs/database.md`, `docs/runbook.md`, `mkdocs.yml` |
| `9e68359` | teste de integração em containers | `.github/workflows/container-integration-test.yml`, `docker/Dockerfile.integration-test`, `docker/docker-compose.integration-test.yml`, `docker/run-container-integration-test.sh` |

## 2026-05-03 — Portal de documentação

### Added

- Portal MkDocs Material com páginas de início, arquitetura, banco, piloto e
  runbook.
- Workflow de publicação automática no GitHub Pages.

### Changed

- Dependências Rust e implementação de leitura Git foram atualizadas.
- Dependências MkDocs passaram a ser controladas por
  `requirements-docs.txt`.
- Metadados do site, copyright e ocultação do gerador foram configurados.

### Traceability

| Commit | Mudança | Arquivos principais |
| --- | --- | --- |
| `5a48eed` | dependências e refactors Git/loader | `Cargo.toml`, `Cargo.lock`, `crates/gitbase-git/src/lib.rs` |
| `800a539` | documentação e deploy MkDocs | `docs/**`, `mkdocs.yml`, `.github/workflows/docs-gh-pages.yml` |
| `d0f7dc6` | requirements do portal | `requirements-docs.txt`, `.github/workflows/docs-gh-pages.yml` |
| `3c9b95c` | refinamento da CI de docs | `requirements-docs.txt`, `.github/workflows/docs-gh-pages.yml` |
| `e74544b` | configuração final do site | `mkdocs.yml` |

## 2026-04-26 — Pipeline GitDatabase funcional

### Added

- Sincronização de repositórios, refs, commits, pais, trees e files.
- Schema `gitbase` e migrations de metadados Git.
- Hidratação lazy de blobs com limite de tamanho e detecção de binário.
- Autenticação do gateway pgwire e hidratação sob demanda por `blob_hash`.
- UAST para Go e Rust com projeções de funções e imports.
- Busca full-text baseada em `tsvector` e função
  `gitbase.search_code(pattern, lang)`.
- Testes de integração para metadata, blobs, busca e UAST.
- CI Rust com fmt, Clippy e testes.
- Hooks pre-commit e política de segurança.

### Changed

- Formatação e APIs internas dos crates foram consolidadas.
- Configurações de indexação ganharam defaults e limites opcionais.
- A chamada de sync nos testes passou a usar slices de paths.

### Removed

- PRD e plano iniciais foram removidos quando deixaram de representar a
  implementação corrente.

### Traceability

| Commit | Mudança | Arquivos principais |
| --- | --- | --- |
| `52884b5` | schema e sync Git | `crates/gitbase-git/**`, `crates/gitbase-loader/**`, `crates/gitbase-db/src/metadata.rs`, `migrations/20260426000001_git_metadata.sql` |
| `ed484fc` | blobs e autenticação pgwire | `crates/gitbase-db/src/blobs.rs`, `crates/gitbase-loader/src/lib.rs`, `crates/gitbase-pgwire/src/handler.rs` |
| `36cbab6` | UAST Go/Rust | `crates/gitbase-uast/**`, `crates/gitbase-db/src/uast.rs`, `migrations/20260426000002_uast.sql` |
| `33597c2` | busca, Dockerfile e guias | `crates/gitbase-db/src/search.rs`, `migrations/20260426000003_search.sql`, `Dockerfile`, `README.md`, `docs/**` |
| `ee01e1f` | CI Rust | `.github/workflows/gitdatabase-ci.yml` |
| `8ae6110` | pre-commit | `.pre-commit-config.yaml`, `README.md` |
| `c9441b4` | política de segurança | `SECURITY.md` |
| `511ee70` | remoção de especificações obsoletas | `plan.md`, `prd.md` |

## 2026-04-19 — Bootstrap do projeto

### Added

- Workspace Rust inicial com CLI, banco e servidor pgwire.
- Migration inicial para habilitar `pg_trgm`.
- PostgreSQL local via Docker Compose.
- Licença MIT e arquivos básicos do repositório.

### Traceability

| Commit | Mudança | Arquivos principais |
| --- | --- | --- |
| `35049b6` | estrutura inicial do repositório | `.gitignore`, `LICENSE`, `README.md` |
| `59777c1` | workspace e esqueleto funcional | `Cargo.toml`, `crates/gitbase-cli/**`, `crates/gitbase-db/**`, `crates/gitbase-pgwire/**`, `docker/docker-compose.yml` |

## Known gaps

As limitações abertas não são tratadas como funcionalidades entregues. Elas estão
descritas e priorizadas em [docs/roadmap.md](docs/roadmap.md), com destaque para:

- alinhamento do toolchain Rust;
- classificação de linguagem;
- sync transacional e reparável;
- reconciliação de refs/force-pushes e foreign keys;
- gestão nativa de repositórios remotos;
- observabilidade e endurecimento do pgwire.
