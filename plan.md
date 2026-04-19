# Gitbase v2 (Rust + PostgreSQL) — plano de desenvolvimento end-to-end

## 1. Objetivo do produto

Construir uma plataforma em Rust que:

- aceite conexões via protocolo PostgreSQL para uso com `psql`, DBeaver, DataGrip e ferramentas compatíveis;
- exponha repositórios Git como tabelas SQL consultáveis;
- trate o Git como fonte da verdade;
- use PostgreSQL como plano de consulta, cache analítico e camada de indexação;
- ofereça UAST, busca textual e carregamento lazy de blobs e artefatos derivados.

## 2. Mudança de direção do plano

Este plano foi ajustado para **começar já usando PostgreSQL desde o início do projeto**.

Isso muda três decisões importantes:

1. o protocolo principal do MVP passa a ser **PostgreSQL**, não MySQL;
2. o banco principal do MVP passa a ser **PostgreSQL real**, não cache local em RocksDB;
3. a engine de execução deixa de ser `DataFusion` no caminho principal, porque o próprio PostgreSQL passa a fazer o trabalho pesado de planner, joins, filtros, índices e agregações.

Em outras palavras: em vez de construir um banco SQL inteiro na unha logo no começo, a ideia passa a ser usar o que o PostgreSQL já faz bem e concentrar o código Rust no que realmente diferencia o produto: **ler Git, hidratar dados sob demanda e extrair inteligência de código**.

## 3. Decisão arquitetural recomendada

### Arquitetura canônica do MVP

- **Protocolo externo:** `pgwire`
- **Banco de dados principal:** PostgreSQL 16+
- **Acesso ao PostgreSQL:** `sqlx`
- **Leitura Git:** `gix`
- **Inspeção leve de queries:** `sqlparser-rs`
- **UAST/AST:** `tree-sitter`
- **CLI e configuração:** `clap`, `serde`, `tracing`
- **Busca textual e índices analíticos:** recursos nativos do PostgreSQL (`tsvector`, `GIN`, `pg_trgm`, `JSONB`)

### Arquitetura lógica

```text
psql / DBeaver / DataGrip
            |
            v
      Rust pgwire server
            |
            v
  Query inspector + lazy loader
            |
            v
        PostgreSQL 16
            |
   -------------------------
   |           |           |
   v           v           v
metadata    UAST cache   search/index
   |           |           |
   --------- Git provider ---------
                    |
                    v
         .git / bare repos / object DB
```

### Por que essa é a melhor direção agora

1. é o menor caminho para ter um sistema útil consultável por SQL real;
2. PostgreSQL já entrega planner, joins, índices, JSONB, views, funções e full-text search;
3. o código Rust pode focar em discovery, ingestão incremental, lazy-load e UAST;
4. clientes como `psql`, DBeaver e DataGrip funcionam naturalmente;
5. a compatibilidade futura com MySQL continua possível por meio de uma ponte `opensrv-mysql` → PostgreSQL, mas deixa de bloquear o MVP.

### O que fica fora do caminho principal

#### MySQL-first no MVP

Não é mais a direção inicial. Pode voltar depois como camada de compatibilidade.

#### DataFusion como engine principal

Não faz mais sentido no caminho crítico se o PostgreSQL já vai executar e otimizar SQL.

#### RocksDB/Tantivy como dependência obrigatória do MVP

Também saem do núcleo inicial. O PostgreSQL passa a cobrir:

- cache persistente;
- índices de texto;
- consultas estruturadas;
- UAST em `JSONB` ou tabelas derivadas.

## 4. Princípios de projeto

1. **Git é a fonte da verdade.**
2. **PostgreSQL é o plano de consulta e cache analítico.**
3. **Nada de checkout para responder query.**
4. **Tudo que puder ser lazy deve ser lazy.**
5. **Dados derivados devem ser deduplicados por `blob_hash`.**
6. **O `pgwire` é a porta de entrada oficial; o acesso direto ao PostgreSQL fica para administração e troubleshooting.**
7. **No MVP, vale mais suportar poucas queries muito bem do que muitas queries mal.**

## 5. Escopo do produto

### MVP

- conexão via protocolo PostgreSQL;
- uso via `psql`, DBeaver e DataGrip;
- `SELECT`, `WHERE`, `LIMIT`, `ORDER BY`, `JOIN` e views;
- schema `gitbase` no PostgreSQL;
- tabelas: `repositories`, `refs`, `commits`, `commit_parents`, `tree_entries`, `files`, `blobs`;
- lazy-load de blobs textuais e artefatos derivados;
- UAST para **Go** e **Rust**;
- projeções `functions` e `imports`;
- função `search_code(pattern, lang)` retornando tabela;
- sincronização inicial e incremental de repositórios.

### Fora do MVP

- compatibilidade com MySQL/Workbench MySQL;
- SIVA;
- escrita em Git ou updates SQL arbitrários;
- suporte amplo a múltiplas linguagens no UAST;
- execução distribuída;
- symbol graph, blame, references e histórico semântico avançado.

## 6. Fluxo principal de execução

### Caminho de uma query

1. cliente conecta via protocolo PostgreSQL ao servidor Rust;
2. o servidor inspeciona a query de forma leve para identificar:
   - quais tabelas do domínio Git foram tocadas;
   - se há filtros úteis como `repository_id`, `commit_hash`, `path`, `blob_hash`;
   - se existe necessidade de hidratar metadados, blobs, UAST ou índice de busca;
3. caso algum dado ainda não esteja disponível no PostgreSQL, o loader busca no Git e persiste;
4. a query é executada no PostgreSQL;
5. o resultado volta ao cliente via `pgwire`.

### Consequência prática

O PostgreSQL vira o banco real de consulta, mas o Rust continua controlando **quando** e **como** os dados chegam lá. É um meio-termo elegante entre “proxy burro” e “engine SQL própria”.

## 7. Componentes principais

| Módulo | Responsabilidade | Crates recomendados |
| --- | --- | --- |
| `gitbase-pgwire` | protocolo PostgreSQL, sessão, autenticação e resposta ao cliente | `pgwire` |
| `gitbase-router` | inspeção leve de query e decisão de hidratação | `sqlparser-rs` |
| `gitbase-db` | acesso ao PostgreSQL, migrations, SQL helpers, pool | `sqlx` |
| `gitbase-git` | leitura de objetos Git sem checkout | `gix` |
| `gitbase-loader` | sync inicial, hidratação lazy, atualização incremental | `sqlx`, `tokio`, tipos próprios |
| `gitbase-uast` | detecção de linguagem, parse `tree-sitter`, normalização e projeções | `tree-sitter` |
| `gitbase-cli` | comandos `serve`, `sync`, `reindex`, configuração e logs | `clap`, `serde`, `tracing` |

## 8. Modelo de dados recomendado

### Estratégia de modelagem

O modelo recomendado é **híbrido-lazy**:

- metadados Git ficam sempre no PostgreSQL;
- blobs ficam no Git como fonte da verdade;
- `blobs.content` é cache lazy;
- UAST e projeções estruturais ficam no PostgreSQL como dados derivados;
- sempre que possível, a deduplicação ocorre por `blob_hash`.

### Schema lógico

Usar um database PostgreSQL com schema `gitbase`.

### Tabelas base

```sql
gitbase.repositories(
  id text primary key,
  name text not null,
  path text not null unique,
  default_ref text,
  is_bare boolean not null,
  discovered_at timestamptz not null
)

gitbase.refs(
  repository_id text not null,
  name text not null,
  target_hash char(40) not null,
  kind text not null,
  primary key (repository_id, name)
)

gitbase.commits(
  repository_id text not null,
  hash char(40) not null,
  tree_hash char(40) not null,
  author_name text,
  author_email text,
  committer_name text,
  committer_email text,
  message text,
  committed_at timestamptz,
  primary key (repository_id, hash)
)

gitbase.commit_parents(
  repository_id text not null,
  commit_hash char(40) not null,
  parent_hash char(40) not null,
  parent_index integer not null,
  primary key (repository_id, commit_hash, parent_index)
)

gitbase.tree_entries(
  repository_id text not null,
  commit_hash char(40) not null,
  path text not null,
  object_hash char(40) not null,
  object_type text not null,
  file_mode text not null,
  size bigint,
  primary key (repository_id, commit_hash, path)
)

gitbase.files(
  repository_id text not null,
  commit_hash char(40) not null,
  path text not null,
  blob_hash char(40) not null,
  language text,
  size bigint,
  is_binary boolean not null,
  primary key (repository_id, commit_hash, path)
)

gitbase.blobs(
  hash char(40) primary key,
  size bigint not null,
  is_binary boolean not null,
  content bytea,
  cached_at timestamptz,
  last_used_at timestamptz
)
```

### Tabelas derivadas por `blob_hash`

```sql
gitbase.uast_cache(
  blob_hash char(40) primary key,
  language text not null,
  uast jsonb not null,
  generated_at timestamptz not null
)

gitbase.uast_functions(
  blob_hash char(40) not null,
  name text not null,
  start_line integer,
  end_line integer,
  signature text,
  primary key (blob_hash, name, start_line)
)

gitbase.uast_imports(
  blob_hash char(40) not null,
  source text not null,
  target text,
  primary key (blob_hash, source, target)
)

gitbase.code_index(
  blob_hash char(40) primary key,
  language text,
  search_vector tsvector not null,
  indexed_at timestamptz not null
)
```

### Views de consulta

```sql
gitbase.functions
gitbase.imports
```

Essas views juntam `files` com `uast_functions` e `uast_imports`, reaproveitando análise por `blob_hash` e expondo o contexto completo de `repository_id`, `commit_hash` e `path`.

### Índices recomendados

- `btree` em `repository_id`, `commit_hash`, `blob_hash`;
- `GIN` com `pg_trgm` em `path`;
- `GIN` em `code_index.search_vector`;
- `GIN` em `uast_cache.uast` caso a UAST crua precise ser consultada diretamente;
- índices compostos para `files(repository_id, commit_hash, path)`.

### Extensões PostgreSQL recomendadas

- `pg_trgm`
- `btree_gin` (opcional)
- `pg_stat_statements` (opcional, mas muito útil para tuning)

## 9. Queries alvo do MVP

```sql
SELECT fn.path, fn.name
FROM gitbase.functions fn
JOIN gitbase.repositories r ON r.id = fn.repository_id
WHERE r.name = 'meu-projeto'
  AND fn.name LIKE 'handle%';
```

```sql
SELECT f.path, b.size
FROM gitbase.files f
JOIN gitbase.blobs b ON b.hash = f.blob_hash
WHERE f.repository_id = 'meu-projeto'
  AND f.path LIKE 'src/%.rs'
LIMIT 20;
```

```sql
SELECT *
FROM gitbase.search_code('TODO', 'go');
```

## 10. Convenção de acesso e sessão

### Caminho oficial

Os clientes devem se conectar ao **servidor Rust (`pgwire`)**, não diretamente ao PostgreSQL, quando precisarem de lazy-load transparente.

### Motivo

Se o cliente falar direto com o PostgreSQL, a camada que decide hidratar dados sob demanda é pulada. Em resumo: o banco fica inteligente, mas o “mordomo” que abastece a geladeira precisa continuar na porta da frente.

### Acesso direto ao PostgreSQL

Pode existir para:

- administração;
- troubleshooting;
- análise offline;
- inspeção manual de índices e tabelas.

Mas não deve ser tratado como o caminho normal do produto.

## 11. Estrutura sugerida do repositório

```text
/Cargo.toml
/crates
  /gitbase-cli
  /gitbase-pgwire
  /gitbase-router
  /gitbase-db
  /gitbase-git
  /gitbase-loader
  /gitbase-uast
/migrations
/tests
  /fixtures
  /integration
  /e2e
/docker
  /docker-compose.yml
/docs
  /adr
```

## 12. Plano de desenvolvimento do início ao fim

## Fase 0 — alinhamento técnico e bootstrap

**Duração sugerida:** 3 a 5 dias

### Objetivo

Fechar o desenho Postgres-first e preparar o ambiente base do projeto.

### Entregáveis

- ADR confirmando a arquitetura `pgwire + PostgreSQL + Git lazy loader`;
- workspace Cargo criado;
- ambiente local com PostgreSQL via Docker Compose;
- migrations vazias e padrão de versionamento definidos;
- fixtures Git definidos para testes.

### Tarefas

- criar ADRs iniciais:
  - ADR-001: PostgreSQL-first no MVP;
  - ADR-002: Git como fonte da verdade;
  - ADR-003: hidratação lazy via `pgwire`;
  - ADR-004: UAST apenas para Go/Rust no MVP;
- definir estratégia de acesso ao PostgreSQL (`sqlx` + pool);
- definir conjunto de queries alvo do MVP;
- escolher fixtures pequenos, médios e grandes.

### Critério de aceite

O time consegue explicar com clareza como uma query sai do `psql`, passa pelo servidor Rust, hidrata dados quando necessário e termina no PostgreSQL.

## Fase 1 — fundação PostgreSQL e `pgwire`

**Duração sugerida:** 1 semana

### Objetivo

Subir a primeira versão consultável do sistema usando protocolo PostgreSQL.

### Entregáveis

- `gitbase serve` funcional;
- conexão via `psql` e DBeaver;
- handshake PostgreSQL via `pgwire`;
- forwarding simples para PostgreSQL real;
- `SELECT 1` e introspecção básica funcionando.

### Tarefas

- implementar servidor `pgwire`;
- configurar pool `sqlx`;
- criar database/schema `gitbase`;
- responder introspecção mínima compatível com clientes mais comuns;
- adicionar logging por conexão e por query.

### Critério de aceite

`psql` conecta no servidor Rust e executa `SELECT 1` com sucesso, com a query sendo servida pelo PostgreSQL de backend.

## Fase 2 — schema, migrations e sync de metadados Git

**Duração sugerida:** 1 a 2 semanas

### Objetivo

Popular o PostgreSQL com os metadados estruturais do Git.

### Entregáveis

- migrations para tabelas base;
- discovery de repositórios bare e não-bare;
- carga de `repositories`, `refs`, `commits`, `commit_parents`, `tree_entries` e `files`;
- comando `gitbase sync`.

### Tarefas

- implementar discovery de repositórios;
- detectar `default_ref`;
- indexar commits e pais corretamente;
- indexar árvores e arquivos sem ler blobs inteiros;
- salvar metadados no PostgreSQL em lotes.

### Critério de aceite

Queries sobre metadados Git funcionam em repositórios reais sem checkout e com suporte correto a merge commits.

## Fase 3 — roteamento de query e hidratação lazy

**Duração sugerida:** 1 a 2 semanas

### Objetivo

Garantir que os dados ausentes sejam carregados automaticamente antes da execução da query.

### Entregáveis

- inspeção leve de queries com `sqlparser-rs`;
- detecção de filtros importantes (`repository_id`, `commit_hash`, `path`, `blob_hash`);
- loader acionado sob demanda;
- controle básico de concorrência para evitar hidratação duplicada.

### Tarefas

- identificar tabelas e predicados relevantes por query;
- decidir quando rodar loaders de blob/UAST/search;
- criar locks/coalescing por `blob_hash` e por repositório/commit;
- integrar hidratação ao fluxo síncrono da query.

### Critério de aceite

Uma query que dependa de dados ainda não carregados consegue completar automaticamente após a hidratação, sem o usuário precisar chamar comandos extras.

## Fase 4 — `blobs` com cache lazy no PostgreSQL

**Duração sugerida:** 1 semana

### Objetivo

Expor conteúdo de arquivo por demanda sem importar o repositório inteiro para o banco.

### Entregáveis

- tabela `blobs` funcional;
- detecção de texto/binário;
- hidratação de `content` sob demanda;
- atualização de `cached_at` e `last_used_at`.

### Tarefas

- ler blob via `gix` usando `blob_hash`;
- persistir conteúdo somente quando fizer sentido;
- impor limite de tamanho para cache textual;
- evitar cache de blobs binários muito grandes;
- medir hit/miss de cache.

### Critério de aceite

A primeira leitura de um blob vem do Git; leituras subsequentes podem vir do PostgreSQL, mantendo o mesmo resultado e melhorando a latência.

## Fase 5 — UAST e projeções estruturais

**Duração sugerida:** 2 semanas

### Objetivo

Adicionar visão estrutural de código usando PostgreSQL como storage dos derivados.

### Entregáveis

- detecção de linguagem;
- parsers `tree-sitter` para Go e Rust;
- tabela `uast_cache`;
- projeções `uast_functions` e `uast_imports`;
- views `gitbase.functions` e `gitbase.imports`.

### Tarefas

- definir modelo interno mínimo de UAST;
- persistir UAST crua em `JSONB` por `blob_hash`;
- extrair `functions` e `imports` como projeções estruturadas;
- reaproveitar análise por `blob_hash` em múltiplos commits.

### Critério de aceite

Consultas sobre funções e imports retornam resultados corretos para projetos Go e Rust sem reparsar o mesmo blob repetidas vezes.

## Fase 6 — busca de código com PostgreSQL

**Duração sugerida:** 1 semana

### Objetivo

Entregar code search útil sem depender de um engine externo no MVP.

### Entregáveis

- tabela `code_index`;
- índices `GIN`/`tsvector`;
- função `gitbase.search_code(pattern, lang)`;
- filtro por linguagem e ranking básico.

### Tarefas

- gerar `search_vector` para blobs textuais;
- indexar incrementalmente por `blob_hash`;
- criar SQL function retornando tabela com `repository_id`, `path`, `blob_hash`, `language`, `score`;
- adicionar fallback controlado para conteúdo ainda não indexado.

### Critério de aceite

`SELECT * FROM gitbase.search_code('func main', 'go')` retorna resultados reais sem varrer todos os blobs manualmente.

## Fase 7 — sync incremental, DX e observabilidade

**Duração sugerida:** 1 a 2 semanas

### Objetivo

Deixar o sistema operável no dia a dia.

### Entregáveis

- comando de sync incremental;
- logs estruturados e métricas;
- documentação de uso;
- mensagens de erro melhores;
- políticas simples de retenção/eviction para cache de blobs.

### Tarefas

- detectar refs novas e commits novos;
- evitar reprocessar blobs já conhecidos;
- expor métricas de tempo por query, tempo de hidratação, hit/miss, parse UAST e indexação;
- documentar fluxo de operação local e em servidor.

### Critério de aceite

O sistema consegue ser executado, observado e atualizado sem depender de adivinhação ou intervenção manual a cada sync.

## Fase 8 — hardening, benchmark e release candidate

**Duração sugerida:** 1 a 2 semanas

### Objetivo

Fechar o MVP com estabilidade e previsibilidade.

### Entregáveis

- suíte de testes automatizada;
- benchmark em datasets reais;
- tuning de pool, concorrência e índices;
- documentação de instalação e troubleshooting.

### Tarefas

- testar concorrência de múltiplos clientes PostgreSQL;
- medir cold start, warm cache, UAST e search;
- revisar locks de hidratação e transações;
- preparar release candidate.

### Critério de aceite

O produto suporta consultas concorrentes de forma estável, tem documentação suficiente e apresenta comportamento previsível em cenários reais.

## 13. Cronograma sugerido

| Semana | Foco |
| --- | --- |
| 1 | Fase 0 |
| 2 | Fase 1 |
| 3 a 4 | Fase 2 |
| 5 a 6 | Fase 3 |
| 7 | Fase 4 |
| 8 a 9 | Fase 5 |
| 10 | Fase 6 |
| 11 | Fase 7 |
| 12 | Fase 8 |

## 14. Estratégia de testes

### Testes unitários

- inspeção de queries;
- detecção de filtros;
- detecção de linguagem;
- deduplicação por `blob_hash`;
- geração de `search_vector`;
- parsing UAST.

### Testes de integração

- migrations no PostgreSQL;
- sync de metadados Git;
- hidratação lazy de blobs;
- criação de views e funções SQL;
- indexação de busca.

### Testes end-to-end

- conexão via `psql`;
- navegação via DBeaver;
- queries alvo do MVP;
- busca de código;
- consulta de `functions` e `imports`.

### Testes de performance

- cold start;
- warm cache de blobs;
- custo de primeira hidratação;
- custo de reuso do mesmo blob em múltiplos commits;
- tempo de resposta do `search_code`.

## 15. Metas não funcionais iniciais

Estas metas são **alvos de engenharia**, não medições já validadas.

- `SELECT 1` via `pgwire`: resposta imediata em ambiente local;
- query com filtros por `repository_id`, `commit_hash` e `path`: sem full scan global desnecessário;
- segunda leitura do mesmo blob: mais rápida que a primeira;
- UAST do mesmo blob: reaproveitada sem reparse desnecessário;
- busca textual: baseada em índice no PostgreSQL, não em varredura completa.

## 16. Principais riscos e mitigação

### 1. Complexidade do roteador de queries

**Risco:** tentar entender SQL demais cedo demais.

**Mitigação:** começar cobrindo apenas os predicados críticos: `repository_id`, `commit_hash`, `path`, `blob_hash`.

### 2. Bypass da camada de hidratação

**Risco:** usuários conectarem direto no PostgreSQL e reclamarem que o lazy-load “sumiu”.

**Mitigação:** documentar `pgwire` como endpoint oficial e usar acesso direto ao PostgreSQL apenas para administração.

### 3. Crescimento do volume de UAST

**Risco:** `JSONB` crescer demais.

**Mitigação:** guardar UAST mínima no MVP e derivar apenas projeções úteis.

### 4. Busca textual insuficiente em algumas linguagens

**Risco:** tokenização nativa do PostgreSQL não ser perfeita para todos os códigos.

**Mitigação:** começar com FTS + trigram e só introduzir engine externa se houver necessidade real.

### 5. Concorrência em hidratação lazy

**Risco:** várias queries tentarem hidratar o mesmo blob ao mesmo tempo.

**Mitigação:** lock/coalescing por `blob_hash` e transações bem definidas.

## 17. Definition of Done do MVP

O MVP estará pronto quando todos os itens abaixo forem verdadeiros:

- `psql` e DBeaver conseguem conectar ao servidor Rust via protocolo PostgreSQL;
- metadados Git são carregados para o PostgreSQL corretamente;
- blobs são hidratados sob demanda sem checkout;
- `functions` e `imports` funcionam ao menos para Go e Rust;
- `search_code(pattern, lang)` retorna resultados reais;
- o fluxo de sync inicial e incremental está documentado;
- existe suíte mínima de testes cobrindo fluxo feliz e casos críticos.

## 18. Evolução após o MVP

Depois que a base estiver estável, as próximas evoluções mais valiosas são:

1. compatibilidade MySQL via ponte `opensrv-mysql` → PostgreSQL;
2. suporte inicial a SIVA;
3. mais linguagens no UAST;
4. procedures/funções mais avançadas como `blame_at`, `history_of(path)` e referências de símbolos;
5. introdução opcional de engine externa de search se PostgreSQL não bastar.

## 19. Próximos passos imediatos

Se fosse para começar agora, a ordem prática seria:

1. subir PostgreSQL 16 em Docker Compose;
2. criar o workspace Cargo e o binário `gitbase`;
3. implementar `pgwire` com `SELECT 1` e forwarding para PostgreSQL;
4. criar migrations do schema `gitbase`;
5. implementar `gitbase sync` para `repositories`, `refs`, `commits`, `commit_parents`, `tree_entries` e `files`;
6. depois entrar em lazy blobs, UAST e `search_code`.

Em resumo: primeiro coloque o PostgreSQL no centro do sistema, depois ensine o Rust a alimentá-lo com inteligência a partir do Git. Menos glamour, menos dor e bem mais chance de chegar rápido em algo usável.
