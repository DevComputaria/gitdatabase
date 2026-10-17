# Plano de funcionalidades ausentes

Este plano foi derivado da implementação atual, das migrations, dos testes e dos
workflows. Ele prioriza correção de dados antes de expansão funcional.

## Princípios de priorização

- **P0:** impede corrupção, operação previsível ou build reproduzível;
- **P1:** completa o ciclo operacional esperado por uma plataforma de indexação;
- **P2:** amplia escala, protocolos e análise semântica;
- cada item deve entregar migration, código, testes, métricas e documentação;
- mudanças de schema ou cache precisam de estratégia de upgrade e rollback.

## Sequência recomendada

```text
P0.1 Toolchain reproduzível
        │
        ├──▶ P0.2 Classificação de arquivos
        │
        └──▶ P0.3 Sync transacional e reparável
                    │
                    └──▶ P0.4 Reconciliação + integridade referencial
                                  │
                                  └──▶ P1.1 Gestão de remotos e jobs
                                                │
                                                ├──▶ P1.2 Observabilidade
                                                └──▶ P1.3 Busca aprimorada
```

## P0 — Fundação de correção

### GD-001 — Alinhar toolchain e build de produção

**Problema:** `rust-version` e o Dockerfile principal usam Rust 1.80, enquanto o
`Cargo.lock` contém crates que exigem Rust 1.89.

**Escopo:**

- definir uma única versão mínima suportada;
- atualizar `Cargo.toml`, Dockerfile, CI e documentação;
- usar `cargo build --locked` em imagens;
- adicionar job que compile explicitamente na MSRV;
- documentar política de atualização do lockfile.

**Critérios de aceite:**

- build local, CI e imagem usam a mesma versão mínima;
- `cargo test --workspace --locked` passa na MSRV;
- dependência que elevar MSRV falha de forma explícita na CI.

**Tamanho:** pequeno. **Dependências:** nenhuma.

### GD-002 — Classificação de arquivo e linguagem

**Problema:** o tree walk grava `language = NULL`, `size = NULL` e
`is_binary = false`. O índice só infere Go/Rust, tornando o filtro `lang`
incompleto.

**Escopo:**

- detector central baseado em extensão e nomes conhecidos;
- preencher linguagem e tamanho no sync quando disponíveis;
- consolidar a classificação binária após hidratação;
- suportar inicialmente C#, JavaScript/TypeScript, Python, Java, Go e Rust;
- backfill versionado para linhas existentes.

**Critérios de aceite:**

- fixtures por linguagem alimentam `files.language` e `code_index.language`;
- `search_code(pattern, lang)` retorna somente a linguagem solicitada;
- linguagens desconhecidas continuam válidas com `NULL`;
- relatório de sync apresenta contagem por classificação.

**Tamanho:** médio. **Dependências:** GD-001.

### GD-003 — Sync transacional, reparável e auditável

**Problema:** statements são confirmados individualmente e commits existentes
pulam a reconstrução de files/tree entries.

**Escopo:**

- transação por repositório;
- staging ou marca de run antes da publicação;
- modo `--repair` para recalcular projeções de commits existentes;
- tabela de runs com status, versão e contadores;
- erro contextualizado por repositório/commit/path.

**Critérios de aceite:**

- falha injetada no meio de um commit não deixa projeções parciais visíveis;
- `--repair` restaura dados removidos intencionalmente em teste;
- duas execuções sem mudança são idempotentes;
- run final registra duração, totais e erro.

**Tamanho:** grande. **Dependências:** GD-001.

### GD-004 — Reconciliação e integridade referencial

**Problema:** refs apagadas, force-pushes e repositórios removidos não são
refletidos no banco; não há foreign keys.

**Escopo:**

- reconciliar o conjunto atual de refs;
- definir política `preserve-history` versus `prune-unreachable`;
- adicionar foreign keys compostas e cascades revisadas;
- garbage collection de blobs, search e UAST sem referências;
- comando `doctor` para detectar órfãos e divergências.

**Critérios de aceite:**

- teste com branch removida elimina ou preserva dados conforme a política;
- teste com force-push não mantém ref apontando para hash antigo;
- migrations validam dados existentes antes de criar constraints;
- `doctor` retorna exit code não zero para inconsistências.

**Tamanho:** grande. **Dependências:** GD-003.

## P1 — Operação como plataforma

### GD-005 — Cadastro e atualização de repositórios remotos

**Problema:** a CLI aceita apenas roots locais; clone/fetch hoje depende de
automação externa.

**Escopo:**

- comandos `repo add`, `repo list`, `repo sync` e `repo remove`;
- URL, branch/refspec, diretório gerenciado e estado do último fetch;
- autenticação por mecanismo externo seguro, sem persistir tokens no banco;
- clone bare e fetch incremental;
- allowlist de hosts e limites de tamanho/tempo.

**Critérios de aceite:**

- um remoto público pode ser cadastrado, clonado, atualizado e indexado;
- credenciais não aparecem em logs, URLs persistidas ou relatórios;
- falhas de rede são classificadas e permitem retry;
- remoção respeita a política de retenção do GD-004.

**Tamanho:** grande. **Dependências:** GD-004.

### GD-006 — Agendamento, fila e concorrência controlada

**Problema:** não há coordenador de jobs ou exclusão mútua por repositório.

**Escopo:**

- fila persistente de sync/hydrate/search/UAST;
- lease por repositório com timeout;
- retries exponenciais para falhas transitórias;
- limites globais e por estágio;
- cancelamento gracioso e retomada.

**Critérios de aceite:**

- dois workers não publicam simultaneamente o mesmo repositório;
- job interrompido expira e pode ser retomado;
- retries não repetem falhas permanentes indefinidamente;
- backlog e duração ficam observáveis.

**Tamanho:** grande. **Dependências:** GD-003, GD-005.

### GD-007 — Observabilidade operacional

**Problema:** existem logs e contadores finais, mas não métricas, traces ou
histórico persistente de execução.

**Escopo:**

- `run_id` em todos os logs;
- métricas de duração, throughput, skips, erros e backlog;
- endpoints separados de liveness/readiness quando em modo serviço;
- dashboard e alertas de referência;
- queries de cobertura publicadas no runbook.

**Critérios de aceite:**

- uma falha pode ser correlacionada do job ao repositório e commit;
- readiness falha quando banco, migration ou roots obrigatórios falham;
- alertas cobrem falha repetida, backlog e queda de cobertura.

**Tamanho:** médio. **Dependências:** GD-003; integra com GD-006.

### GD-008 — Busca por snapshot e resultados não duplicados

**Problema:** `search_code` pode retornar o mesmo blob para muitos commits e não
oferece filtro direto por repositório/ref/path.

**Escopo:**

- API SQL versionada com filtros por repositório, ref, commit e path;
- modo `latest` baseado no commit alvo da ref;
- destaque de trechos e linha aproximada;
- paginação estável;
- plano de compatibilidade para a função atual.

**Critérios de aceite:**

- busca em `latest` retorna uma ocorrência por path do snapshot;
- filtros combinados usam índices verificados por `EXPLAIN`;
- paginação não perde nem duplica resultados;
- benchmark define limite de latência para corpus de referência.

**Tamanho:** grande. **Dependências:** GD-002, GD-004.

## P2 — Expansão semântica e protocolo

### GD-009 — UAST versionada e multilíngue

- versionar parser/schema no cache e invalidar somente dados incompatíveis;
- adicionar C#, TypeScript/JavaScript e Python;
- projetar tipos, chamadas e dependências além de funções/imports;
- criar fixtures reais e métricas de erro por linguagem.

**Aceite resumido:** rebuild seletivo por versão, três novas linguagens e
compatibilidade documentada. **Tamanho:** grande. **Dependências:** GD-002.

### GD-010 — Endurecimento do pgwire

- TLS e autenticação sem senha cleartext;
- autorização read-only por padrão;
- protocolo estendido/prepared statements;
- streaming de linhas com backpressure;
- parser SQL para classificação e hidratação sob demanda.

**Aceite resumido:** clientes comuns conectam com TLS, usuário read-only não
executa DDL/DML e consultas grandes não são materializadas integralmente.
**Tamanho:** grande. **Dependências:** GD-007.

### GD-011 — Retenção, privacidade e exclusão

- políticas por repositório para conteúdo, histórico e caches;
- detecção opcional de segredos antes da hidratação;
- exclusão verificável e relatório de impacto;
- auditoria de acesso a conteúdo.

**Aceite resumido:** remoção elimina todas as projeções cobertas pela política e
gera evidência auditável sem revelar o conteúdo. **Tamanho:** grande.
**Dependências:** GD-004, GD-005.

## Primeiro incremento recomendado

O primeiro ciclo deve entregar GD-001 e GD-002, seguido do esqueleto de run do
GD-003. Esse corte elimina a divergência de build, torna a busca por linguagem
útil e cria a base observável para alterar a semântica de sincronização.

### Entregáveis do ciclo

1. toolchain 1.89 alinhada em código, container, CI e docs;
2. módulo compartilhado de detecção de linguagem;
3. migration de backfill de `files.language`;
4. testes E2E com filtro por `csharp` e `rust`;
5. tabela `index_runs` e `run_id` nos logs;
6. ADR definindo identidade de repositório e política de reconciliação.

## Definição de pronto comum

Uma funcionalidade do roadmap só está pronta quando:

- migrations forward foram testadas com dados existentes;
- testes unitários, integração e container cobrem sucesso e falha;
- métricas e logs não expõem conteúdo ou credenciais;
- runbook contém implantação, rollback e troubleshooting;
- arquitetura e modelo de dados foram atualizados;
- impacto de compatibilidade e reindexação está explícito no changelog.
