# PRD: Gitbase v2

## 1. Visão geral do produto

### 1.1 Título e versão do documento

- PRD: Gitbase v2
- Versão: 0.1

### 1.2 Resumo do produto

O projeto é uma plataforma interna para consulta e exploração de repositórios Git via SQL, com foco equilibrado em três frentes: metadados Git, busca de código e inteligência estrutural baseada em UAST. A solução será construída em Rust, com arquitetura Postgres-first, expondo um endpoint compatível com o protocolo PostgreSQL para uso via `psql`, DBeaver e DataGrip.

A primeira entrega deve suportar cerca de 100 repositórios e poucos usuários simultâneos, priorizando confiabilidade operacional, setup simples e consultas realmente úteis para engenharia. O Git continuará sendo a fonte da verdade, enquanto o PostgreSQL será a camada de consulta, indexação e cache persistente para dados derivados e blobs carregados sob demanda.

O objetivo do MVP não é substituir todas as ferramentas de análise de código, mas criar uma base sólida e pragmática para responder perguntas sobre código e histórico com SQL, reduzir o tempo de investigação de repositórios e abrir caminho para recursos mais avançados em versões futuras.

## 2. Objetivos

### 2.1 Objetivos de negócio

- Criar um produto interno que reduza o tempo para responder perguntas sobre código, histórico e estrutura de repositórios.
- Validar uma arquitetura Postgres-first capaz de atender o cenário inicial de 100 repositórios com poucos usuários concorrentes.
- Estabelecer uma fundação reutilizável para evoluções futuras, incluindo compatibilidade MySQL, linguagens adicionais e consultas semânticas mais profundas.
- Reduzir dependência de scripts ad hoc, grep manual e consultas manuais em múltiplos repositórios.
- Melhorar a produtividade de times de plataforma, backend e liderança técnica em análises cross-repo.

### 2.2 Objetivos do usuário

- Conectar ao sistema com ferramentas SQL conhecidas, sem precisar aprender uma interface proprietária.
- Consultar rapidamente repositórios, refs, commits, árvores, arquivos e blobs.
- Fazer busca textual em código com filtro por linguagem.
- Consultar funções e imports em Go e Rust sem navegar manualmente por dezenas de repositórios.
- Ter setup previsível em ambiente local e em produção, com Docker, linting e observabilidade básicos.

### 2.3 Não objetivos

- Suportar escrita em Git, update arbitrário via SQL ou edição de repositórios pelo produto no MVP.
- Entregar compatibilidade com MySQL como requisito da primeira versão.
- Suportar execução distribuída ou multi-tenant no MVP.
- Oferecer UAST completa para um grande número de linguagens logo na primeira entrega.
- Cobrir todos os dialetos SQL ou todo o comportamento nativo do PostgreSQL além do necessário para o produto.
- Substituir ferramentas de code review, IDE ou análise estática já existentes.

## 3. Personas de usuário

### 3.1 Tipos principais de usuário

- Desenvolvedor de backend ou plataforma
- Engenheiro de dados ou analista técnico
- Tech lead ou arquiteto
- Operador de plataforma ou DevOps

### 3.2 Detalhes básicos das personas

- **Desenvolvedor de backend/plataforma**: quer localizar arquivos, funções, imports e padrões de código em múltiplos repositórios sem abrir cada projeto manualmente.
- **Engenheiro de dados/analista técnico**: quer usar SQL para responder perguntas estruturadas sobre commits, arquivos, dependências e padrões de código.
- **Tech lead/arquiteto**: quer entender impacto de mudanças, distribuição de padrões e dependências entre repositórios.
- **Operador de plataforma/DevOps**: quer instalar, atualizar, monitorar e diagnosticar o produto com baixo esforço operacional.

### 3.3 Acesso baseado em papéis

- **Administrador**: configura repositórios, executa sync inicial e incremental, gerencia credenciais, ajusta parâmetros operacionais e acessa observabilidade.
- **Analista/Desenvolvedor**: executa consultas de leitura, busca de código e exploração de UAST/views derivadas, sem permissão de alterar configuração do sistema.
- **Operador**: implanta a solução, gerencia containers, banco, logs, métricas e políticas de retenção, com acesso administrativo ao ambiente.

## 4. Requisitos funcionais

- **Acesso SQL via PostgreSQL** (Prioridade: P0)
  - O sistema deve expor um endpoint compatível com protocolo PostgreSQL usando `pgwire`.
  - O produto deve permitir conexão via `psql`, DBeaver e DataGrip.
  - O endpoint deve suportar autenticação e perfis de acesso ao menos para leitura e administração.
  - O sistema deve responder introspecção suficiente para uso nas ferramentas cliente suportadas.

- **Sincronização de repositórios Git** (Prioridade: P0)
  - O produto deve descobrir e registrar repositórios bare e não-bare a partir de diretórios configurados.
  - O sistema deve executar sync inicial para popular metadados Git em PostgreSQL.
  - O sistema deve executar sync incremental, evitando reprocessamento desnecessário.
  - Falhas em um repositório não devem bloquear a sincronização dos demais.

- **Modelo consultável de metadados Git** (Prioridade: P0)
  - O sistema deve expor tabelas para `repositories`, `refs`, `commits`, `commit_parents`, `tree_entries`, `files` e `blobs`.
  - O modelo deve suportar merge commits por meio de `commit_parents`.
  - O sistema deve permitir consultas filtradas por `repository_id`, `commit_hash`, `path` e `blob_hash`.

- **Carregamento lazy de blobs** (Prioridade: P0)
  - O conteúdo de blobs deve ser carregado sob demanda a partir do Git.
  - O PostgreSQL deve armazenar cache persistente apenas quando fizer sentido operacional.
  - Arquivos binários e blobs grandes devem ter política de proteção para evitar degradação do sistema.
  - Leituras repetidas do mesmo blob devem reutilizar conteúdo já carregado.

- **Busca textual em código** (Prioridade: P1)
  - O produto deve expor uma função SQL `search_code(pattern, lang)` retornando tabela.
  - A busca deve suportar filtro por linguagem e ranking básico de resultados.
  - O índice deve ser baseado nos recursos nativos do PostgreSQL (`tsvector`, `GIN`, `pg_trgm`) no MVP.

- **Inteligência estrutural com UAST** (Prioridade: P1)
  - O sistema deve gerar UAST mínima para Go e Rust.
  - O sistema deve armazenar UAST derivada por `blob_hash` em PostgreSQL.
  - O produto deve expor visões estruturadas de `functions` e `imports`.
  - O reuso por `blob_hash` deve evitar reparsing desnecessário em commits diferentes.

- **Experiência operacional e setup** (Prioridade: P0)
  - O projeto deve ter setup de desenvolvimento reproduzível com Docker Compose.
  - O produto deve ter setup inicial de produção baseado em container e PostgreSQL 16.
  - O sistema deve ter imagem Docker oficial para build e execução.
  - O projeto deve definir linting e padrões mínimos de qualidade antes do MVP ser considerado pronto.

- **Observabilidade e diagnósticos** (Prioridade: P1)
  - O produto deve expor logs estruturados, health check e métricas básicas.
  - O sistema deve produzir mensagens de erro claras para queries não suportadas, parse inválido, autenticação falha e sync interrompido.
  - O operador deve conseguir identificar gargalos de sync, hidratação de blob, indexação e parsing UAST.

## 5. Experiência do usuário

### 5.1 Pontos de entrada e fluxo de primeiro uso

- Clonar o projeto e subir o ambiente local com Docker Compose.
- Configurar variáveis de ambiente para o serviço Rust e para o PostgreSQL.
- Executar migrations do banco e registrar o diretório raiz com os repositórios Git.
- Rodar `gitbase sync` para popular metadados iniciais.
- Conectar com `psql`, DBeaver ou DataGrip.
- Executar consultas básicas em `repositories`, `files`, `functions` e `search_code`.

### 5.2 Experiência central

- **Conectar ao serviço**: o usuário usa uma ferramenta SQL familiar para acessar o produto.
  - Isso reduz fricção de adoção e dispensa curva de aprendizado de interface proprietária.

- **Explorar metadados Git**: o usuário consulta repositórios, branches, commits e arquivos.
  - Isso permite responder perguntas operacionais e históricas com rapidez.

- **Consultar conteúdo sob demanda**: o usuário acessa blobs e conteúdo de arquivos somente quando necessário.
  - Isso preserva desempenho e evita ingestão desnecessária de todo o conteúdo.

- **Buscar código por padrão textual**: o usuário procura trechos relevantes sem depender de scripts externos.
  - Isso acelera investigações e triagens cross-repo.

- **Explorar estrutura de código**: o usuário consulta funções e imports para Go e Rust.
  - Isso ajuda a mapear responsabilidades, dependências e impacto arquitetural.

- **Atualizar o índice após mudanças**: o operador ou administrador executa sync incremental.
  - Isso mantém o ambiente atualizado sem precisar reconstruir tudo do zero.

### 5.3 Recursos avançados e casos de borda

- Consultas sobre linguagens não suportadas devem continuar retornando metadados de arquivo mesmo sem UAST.
- Blobs binários ou acima do limite configurado devem retornar metadados e comportamento seguro, sem travar a consulta.
- Múltiplas queries simultâneas para o mesmo blob devem ser coalescidas para evitar hidratação duplicada.
- Falhas de parsing em um arquivo não devem derrubar a consulta de todo o lote; o sistema deve isolar e registrar o erro.
- Repositórios com problemas de leitura devem ser marcados com erro operacional sem impedir o uso dos repositórios saudáveis.

### 5.4 Destaques de UI/UX

- Experiência SQL-first, sem UI web obrigatória no MVP.
- Compatibilidade explícita com `psql`, DBeaver e DataGrip.
- Schema `gitbase` consistente e previsível.
- Erros acionáveis, com linguagem adequada para operadores e analistas.
- Fluxo de setup e operação padronizado por Docker, migrations e comandos CLI.

## 6. Narrativa

Um desenvolvedor ou analista conecta ao Gitbase v2 usando uma ferramenta SQL já conhecida, sincroniza cerca de 100 repositórios e começa a fazer perguntas reais sobre histórico, arquivos, funções e padrões de código. Quando uma consulta precisa de conteúdo ou UAST ainda não carregados, o serviço em Rust hidrata os dados a partir do Git, persiste o necessário em PostgreSQL e devolve a resposta sem exigir etapas manuais extras. O resultado é uma experiência prática e incremental: rápida para começar, previsível para operar e útil para investigações técnicas do dia a dia.

## 7. Métricas de sucesso

### 7.1 Métricas centradas no usuário

- Tempo para um usuário técnico sair de ambiente limpo até a primeira query bem-sucedida em ambiente local: até 30 minutos.
- Tempo para conectar uma ferramenta suportada (`psql`, DBeaver ou DataGrip) e listar tabelas do schema `gitbase`: até 5 minutos após ambiente estar pronto.
- Tempo de retorno do primeiro conjunto de resultados de `search_code` em ambiente aquecido: p95 abaixo de 2 segundos.
- Tempo de retorno de consultas indexadas por `repository_id`, `commit_hash` e `path`: p95 abaixo de 1 segundo em ambiente aquecido.

### 7.2 Métricas de negócio

- Validar o MVP com um conjunto inicial de 100 repositórios internos.
- Sustentar uso inicial por um grupo pequeno de usuários técnicos, com meta de até 10 usuários concorrentes no piloto.
- Reduzir em pelo menos 50% o tempo médio para responder perguntas cross-repo hoje feitas manualmente.
- Obter uso recorrente semanal do piloto por pelo menos dois perfis distintos: desenvolvimento e plataforma.

### 7.3 Métricas técnicas

- `SELECT 1` via endpoint `pgwire`: p95 abaixo de 200 ms em ambiente aquecido.
- Leitura de blob textual em cache para arquivos de até 1 MB: p95 abaixo de 200 ms.
- Reuso de análise por `blob_hash` para UAST e blobs já aquecidos: taxa alvo de reaproveitamento acima de 80% após uso contínuo do piloto.
- Sync incremental não deve reprocessar blobs e UAST já conhecidos sem necessidade.
- Falhas de parsing ou hidratação devem ser observáveis por logs e métricas, sem comprometer a disponibilidade global do serviço.

## 8. Considerações técnicas

### 8.1 Pontos de integração

- Sistema de arquivos com repositórios Git bare e não-bare.
- PostgreSQL 16 como banco principal do produto.
- Clientes SQL compatíveis com PostgreSQL (`psql`, DBeaver, DataGrip).
- CLI operacional para `serve`, `sync` e `reindex`.
- Docker Compose para desenvolvimento e imagem Docker para execução padronizada.
- Pipeline de CI para lint, testes e build da imagem.

### 8.2 Armazenamento de dados e privacidade

- O conteúdo consultado é código-fonte interno e deve ser tratado como dado sensível da organização.
- O Git permanece como fonte da verdade; o PostgreSQL armazena metadados, cache de blobs e dados derivados.
- Blobs devem ser persistidos sob demanda, com limites configuráveis por tamanho e tipo.
- Credenciais e segredos devem ser injetados por variáveis de ambiente ou secret manager, nunca hardcoded.
- O MVP não deve depender de processamento externo de código em serviços de terceiros.

### 8.3 Escalabilidade e performance

- O escopo inicial é 100 repositórios e poucos usuários concorrentes, com meta operacional de até 10 usuários simultâneos.
- A arquitetura deve privilegiar sync incremental, deduplicação por `blob_hash` e hidratação lazy.
- Índices devem ser criados para consultas frequentes por repositório, commit, caminho, blob e busca textual.
- O design deve permitir separar aplicação e PostgreSQL em serviços distintos já no MVP, mesmo sem necessidade de cluster.
- O produto deve ser otimizado primeiro para previsibilidade operacional, depois para throughput máximo.

### 8.4 Desafios potenciais

- Encontrar o equilíbrio certo de inspeção de query sem tentar reimplementar um parser SQL completo no serviço.
- Manter o volume de UAST sob controle sem sacrificar utilidade das consultas.
- Evitar contenção em hidratação lazy quando múltiplas queries pedirem o mesmo blob ao mesmo tempo.
- Lidar com diferenças de tokenização e relevância da busca textual para linguagens de programação.
- Evitar confusão operacional entre o endpoint oficial via `pgwire` e o acesso administrativo direto ao PostgreSQL.

### 8.5 Stack sugerida

- **Linguagem principal**: Rust estável.
- **Protocolo SQL externo**: `pgwire`.
- **Banco de dados**: PostgreSQL 16.
- **Acesso ao banco**: `sqlx`.
- **Leitura de Git**: `gix`.
- **Inspeção leve de queries**: `sqlparser-rs`.
- **AST/UAST**: `tree-sitter` com parsers iniciais para Go e Rust.
- **CLI e configuração**: `clap`, `serde`, `tracing`.
- **Observabilidade**: `tracing`, logs estruturados e métricas exportáveis.
- **Busca textual**: `tsvector`, `GIN`, `pg_trgm` e funções SQL no PostgreSQL.

### 8.6 Setup de desenvolvimento

- Ambiente local baseado em Docker Compose com, no mínimo:
  - serviço `postgres` (PostgreSQL 16);
  - serviço `gitbase` em modo dev;
  - volume(s) montado(s) para diretórios de repositórios Git de teste.
- Fluxo padrão de desenvolvimento:
  - subir serviços de infraestrutura;
  - aplicar migrations;
  - executar `gitbase sync` com repositórios de fixture;
  - conectar com `psql` ou DBeaver;
  - rodar testes e lint antes de commit.
- O modo dev deve permitir hot reload simples ou rebuild incremental do binário Rust.
- Fixtures de teste devem incluir repositórios pequenos e médios com exemplos em Go e Rust.

### 8.7 Setup de produção

- Produção inicial baseada em container, com a aplicação e o PostgreSQL em serviços separados.
- O PostgreSQL pode ser autogerenciado ou serviço gerenciado compatível, desde que suporte as extensões necessárias.
- O deployment inicial deve priorizar simplicidade operacional: uma VM ou pequeno cluster de containers é suficiente para o cenário de poucos usuários.
- Logs devem sair em `stdout/stderr` e métricas devem estar disponíveis para coleta.
- Backups do PostgreSQL e políticas de retenção devem ser definidos antes do piloto em produção.
- O rollout deve suportar atualização controlada do binário, migrations versionadas e rollback operacional simples.

### 8.8 Qualidade, lint e imagem Docker

- **Lint obrigatório de Rust**:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- **Qualidade de SQL/migrations**:
  - padronização de migrations com revisão automatizada e lint SQL usando `sqlfluff` ou `sqruff`.
- **Testes recomendados no pipeline**:
  - unitários;
  - integração com PostgreSQL real;
  - end-to-end com cliente SQL.
- **Imagem Docker de desenvolvimento**:
  - baseada em imagem oficial de Rust com toolchain e dependências de build.
- **Imagem Docker de runtime**:
  - build multi-stage;
  - estágio final enxuto, preferencialmente `debian:bookworm-slim` no MVP por simplicidade operacional e depuração;
  - evolução futura para imagem distroless após estabilização operacional.

## 9. Marcos e sequenciamento

### 9.1 Estimativa do projeto

- Tamanho: Médio a grande
- Estimativa: 10 a 12 semanas para MVP operacional

### 9.2 Tamanho e composição do time

- Time sugerido: 3 a 4 pessoas
- Papéis envolvidos:
  - 1 engenheiro backend Rust
  - 1 engenheiro backend/plataforma com foco em PostgreSQL
  - 1 engenheiro DevOps/plataforma parcial
  - 1 PM/QA/tech lead parcial para priorização e validação

### 9.3 Fases sugeridas

- **Fase 1**: fundação, setup e conectividade PostgreSQL (2 semanas)
  - Entregáveis principais: workspace Rust, Docker Compose, PostgreSQL 16, `pgwire`, `SELECT 1`, schema inicial e pipeline básico de lint/teste.

- **Fase 2**: sync de metadados Git e modelo consultável (2 semanas)
  - Entregáveis principais: discovery de repositórios, migrations, `repositories`, `refs`, `commits`, `commit_parents`, `tree_entries`, `files` e comando `sync`.

- **Fase 3**: hidratação lazy de blobs e segurança operacional (2 semanas)
  - Entregáveis principais: tabela `blobs`, cache lazy, limites para blobs binários/grandes, autenticação mínima, logs e health check.

- **Fase 4**: UAST e projeções estruturais (2 a 3 semanas)
  - Entregáveis principais: suporte a Go/Rust, `uast_cache`, `functions`, `imports` e reaproveitamento por `blob_hash`.

- **Fase 5**: busca de código, hardening e piloto (2 a 3 semanas)
  - Entregáveis principais: `search_code`, índices de busca, sync incremental, métricas, documentação, imagem Docker oficial e validação com 100 repositórios.

## 10. Histórias de usuário

### 10.1 Configurar o ambiente local

- **ID**: GB-001
- **Descrição**: Como desenvolvedor da plataforma, quero subir o ambiente local com Docker Compose para começar a desenvolver e testar o produto rapidamente.
- **Critérios de aceitação**:
  - O projeto fornece arquivos de configuração para subir PostgreSQL e o serviço principal em ambiente local.
  - O setup local inclui instruções claras para migrations, sync e conexão via cliente SQL.
  - Um novo colaborador consegue executar a primeira query após seguir o guia de setup.

### 10.2 Conectar por um cliente SQL suportado

- **ID**: GB-002
- **Descrição**: Como usuário técnico, quero conectar com `psql`, DBeaver ou DataGrip para consultar o produto sem depender de interface proprietária.
- **Critérios de aceitação**:
  - O endpoint `pgwire` aceita conexões autenticadas dos clientes suportados.
  - O usuário consegue listar schemas e tabelas relevantes do produto.
  - O sistema responde com mensagens claras em caso de credenciais inválidas ou cliente incompatível.

### 10.3 Registrar e sincronizar repositórios

- **ID**: GB-003
- **Descrição**: Como administrador, quero registrar diretórios com repositórios Git e rodar um sync inicial para tornar os dados consultáveis.
- **Critérios de aceitação**:
  - O sistema descobre repositórios bare e não-bare em diretórios configurados.
  - O sync inicial popula `repositories`, `refs`, `commits`, `commit_parents`, `tree_entries` e `files`.
  - Falhas em um repositório são registradas sem interromper totalmente os demais.

### 10.4 Consultar metadados Git

- **ID**: GB-004
- **Descrição**: Como analista técnico, quero consultar metadados Git por SQL para responder perguntas sobre histórico, refs e arquivos.
- **Critérios de aceitação**:
  - O usuário consegue consultar `repositories`, `refs`, `commits` e `tree_entries` com filtros por repositório e commit.
  - Merge commits aparecem corretamente por meio de `commit_parents`.
  - Consultas indexadas retornam resultados dentro das metas de desempenho definidas para o MVP.

### 10.5 Consultar arquivos e blobs sob demanda

- **ID**: GB-005
- **Descrição**: Como desenvolvedor, quero consultar arquivos e seu conteúdo sob demanda para investigar código sem importar tudo antecipadamente.
- **Critérios de aceitação**:
  - O sistema retorna metadados de `files` sem exigir carregamento completo de todos os blobs.
  - Quando o conteúdo é solicitado, o blob é carregado do Git e pode ser reutilizado em consultas futuras.
  - O sistema aplica limites de tamanho e tratamento apropriado para blobs binários.

### 10.6 Buscar código por termo e linguagem

- **ID**: GB-006
- **Descrição**: Como desenvolvedor, quero buscar trechos de código usando SQL para localizar padrões e ocorrências relevantes em múltiplos repositórios.
- **Critérios de aceitação**:
  - A função `search_code(pattern, lang)` retorna tabela com caminho, repositório, blob e score.
  - O usuário pode filtrar por linguagem no MVP.
  - A busca usa índice e não depende de varredura completa de todos os blobs em ambiente aquecido.

### 10.7 Consultar funções e imports

- **ID**: GB-007
- **Descrição**: Como tech lead, quero consultar funções e imports para analisar estrutura de código em Go e Rust.
- **Critérios de aceitação**:
  - O sistema expõe `functions` e `imports` com contexto de repositório, commit e path.
  - O mesmo `blob_hash` não é reparsado desnecessariamente em commits diferentes.
  - Arquivos com parsing inválido não derrubam a consulta inteira e geram diagnóstico observável.

### 10.8 Atualizar dados após mudanças nos repositórios

- **ID**: GB-008
- **Descrição**: Como administrador, quero executar sync incremental para manter o índice atualizado sem reconstruir tudo do zero.
- **Critérios de aceitação**:
  - O sistema detecta refs e commits novos desde a última sincronização.
  - Blobs e UAST já conhecidos não são reprocessados sem necessidade.
  - O resultado do sync incremental fica visível por logs e métricas.

### 10.9 Lidar com blobs grandes, binários e linguagens não suportadas

- **ID**: GB-009
- **Descrição**: Como usuário técnico, quero comportamento seguro para arquivos especiais para que o sistema não falhe em cenários de borda.
- **Critérios de aceitação**:
  - Blobs acima do limite configurado não causam falha geral do serviço.
  - Arquivos binários são identificados corretamente e tratados com política apropriada.
  - Arquivos de linguagens não suportadas continuam disponíveis ao menos via metadados e conteúdo, quando permitido.

### 10.10 Receber erros claros e acionáveis

- **ID**: GB-010
- **Descrição**: Como usuário ou operador, quero mensagens de erro claras para entender falhas de autenticação, parse, sync ou query.
- **Critérios de aceitação**:
  - Erros comuns retornam mensagens compreensíveis e contexto suficiente para troubleshooting.
  - Falhas operacionais são registradas com correlação mínima para investigação.
  - O sistema diferencia erros de autenticação, erro de query, erro de parsing e erro de sincronização.

### 10.11 Observar saúde e desempenho do serviço

- **ID**: GB-011
- **Descrição**: Como operador, quero acompanhar logs, health check e métricas para operar o sistema com confiança.
- **Critérios de aceitação**:
  - O serviço expõe health check para readiness/liveness.
  - Logs estruturados permitem identificar sync, hidratação, parsing UAST e queries lentas.
  - Métricas mínimas de latência, falha, cache hit/miss e sync estão disponíveis.

### 10.12 Controlar acesso e proteger dados internos

- **ID**: GB-012
- **Descrição**: Como administrador, quero controlar autenticação e perfis de acesso para proteger código-fonte e operação do produto.
- **Critérios de aceitação**:
  - O sistema exige autenticação para conexões ao endpoint oficial.
  - Perfis de leitura e administração são separados no MVP.
  - Segredos e credenciais não ficam hardcoded no código ou em arquivos versionados.

### 10.13 Publicar e operar o produto com imagem Docker oficial

- **ID**: GB-013
- **Descrição**: Como operador de plataforma, quero uma imagem Docker oficial para padronizar build, deploy e operação em dev e produção.
- **Critérios de aceitação**:
  - O projeto publica uma imagem de runtime versionada para o serviço principal.
  - O build usa estratégia multi-stage para reduzir tamanho e superfície de ataque.
  - A imagem é compatível com o setup de desenvolvimento e com o deployment inicial em produção.

### 10.14 Validar qualidade com lint e testes automatizados

- **ID**: GB-014
- **Descrição**: Como mantenedor do produto, quero lint e testes automatizados para reduzir regressões e manter a base saudável.
- **Critérios de aceitação**:
  - O pipeline executa `cargo fmt --check` e `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
  - O pipeline inclui testes unitários, de integração com PostgreSQL e end-to-end com cliente SQL.
  - Mudanças que quebrem lint, build ou testes bloqueiam a promoção do artefato do MVP.
