# Atributos de qualidade e riscos

Este documento registra como a implementação atual atende requisitos não
funcionais e quais controles precisam evoluir antes de uso em produção.

## Resumo executivo

| Atributo | Estado atual | Risco principal | Próximo controle |
| --- | --- | --- | --- |
| Consistência | eventual entre estágios | sync parcial e dados obsoletos | transação + reconciliação |
| Disponibilidade | comandos reiniciáveis | dependência de filesystem e PostgreSQL | checkpoints e retries classificados |
| Segurança | credencial pgwire simples | cleartext e SQL amplo | TLS + privilégio mínimo + autorização |
| Performance | deduplicação por blob e índices GIN | histórico completo por commit | escopo por refs e processamento concorrente |
| Observabilidade | logs estruturados e relatórios de CI | sem métricas/traces | métricas por estágio e tabela de runs |
| Portabilidade | Rust + PostgreSQL + Docker | toolchain declarado divergente | pin único e verificação de MSRV |
| Testabilidade | unitários, integração DB e E2E | cenários destrutivos ausentes | testes de force-push, falha parcial e upgrade |

## Consistência e integridade

### Garantias existentes

- chaves primárias evitam duplicidade das entidades principais;
- upserts tornam os caminhos felizes repetíveis;
- blobs e índices semânticos são deduplicados por hash;
- migrations são aplicadas antes de cada operação;
- o teste de containers valida o pipeline completo contra um repositório real.

### Riscos

- ausência de foreign keys permite referências órfãs;
- sync não é atômico por repositório;
- remoções no Git não são propagadas;
- caches não carregam versão do algoritmo/parser;
- commits existentes impedem reparo automático de suas projeções.

### Meta proposta

Uma execução deve produzir um snapshot internamente consistente ou não publicar
nenhuma alteração para aquele repositório. A reconciliação deve distinguir
histórico preservado de objetos inalcançáveis que podem ser removidos.

## Segurança

### Dados processados

Código-fonte pode conter segredos, dados pessoais e propriedade intelectual. O
banco replica conteúdo textual de blobs e amplia sua superfície de acesso.

### Controles existentes

- limite de tamanho para hidratação;
- binários não são persistidos como conteúdo;
- roots permitidos restringem a origem de blobs;
- usuário e senha pgwire são configuráveis;
- o workflow usa somente permissão `contents: read`.

### Controles ausentes

- TLS nativo no listener pgwire;
- hash/armazenamento seguro da senha do gateway;
- autorização por repositório, schema ou tipo de statement;
- redaction explícita de SQL e paths nos logs;
- secret scanning antes de persistir conteúdo;
- política de retenção e exclusão verificável.

O gateway não deve ser exposto à internet no estado atual. Em produção, use rede
privada, backend PostgreSQL com privilégios mínimos e proteção de transporte na
frente do listener.

## Performance e capacidade

### Características atuais

- o tree walk é realizado para cada commit novo;
- files e tree entries são armazenados por commit;
- processamento de repositórios e blobs é sequencial;
- pgwire usa `fetch_all`, mantendo o resultado completo em memória;
- GIN acelera full-text search e trigrams aceleram paths;
- deduplicação de conteúdo reduz armazenamento quando blobs se repetem.

### Variáveis de capacidade

| Variável | Impacto |
| --- | --- |
| commits alcançáveis | tempo de sync e linhas em files/tree entries |
| arquivos por commit | maior multiplicador de armazenamento |
| blobs únicos | hidratação, busca e cache UAST |
| tamanho máximo do blob | memória, I/O e tamanho do banco |
| conexões do pool | concorrência com PostgreSQL |
| cardinalidade da busca | memória do processo pgwire |

Antes de definir SLOs, deve-se criar um benchmark com repositórios pequeno,
médio e grande, registrando tempo, CPU, RSS, I/O e crescimento das tabelas.

## Disponibilidade e recuperação

O GitDatabase não mantém estado de execução fora das tabelas de domínio. A
recuperação é baseada em repetir comandos e, em alguns casos, truncar caches.

Requisitos recomendados:

- backup consistente do PostgreSQL antes de migrations destrutivas;
- retenção independente dos repositórios Git de origem;
- readiness que verifique migrations e acesso aos roots;
- timeout e cancelamento gracioso por estágio;
- registro persistente de runs, duração, contadores e erro final;
- runbook de restore testado periodicamente.

## Observabilidade

`tracing` fornece eventos de conexão, migration e totais por comando. Ainda
faltam correlação e métricas operacionais.

Modelo mínimo proposto para uma run:

- `run_id`, comando, início, fim e status;
- repositório e ref alvo;
- contadores lidos, inseridos, atualizados, removidos e ignorados;
- erro classificado e retryable;
- versão do binário, schema, parser e configuração efetiva;
- duração de descoberta, leitura Git, banco e indexação.

## Compatibilidade e upgrades

- o package declara MSRV 1.80, mas o lock atual exige Rust 1.89;
- migrations são somente forward e não há procedimento de downgrade;
- caches UAST/search não registram versão do gerador;
- mudanças em `search_vector` ou parsers exigem reindexação manual.

Uma release deve publicar matriz de compatibilidade com Rust, PostgreSQL e schema,
além de marcar quando um upgrade exige rebuild de cache.

## Estratégia de testes alvo

| Camada | Cobertura atual | Cobertura a adicionar |
| --- | --- | --- |
| unidade | UAST Go/Rust | detecção de linguagem, parser SQL/hashes |
| integração | sync, blobs, search, UAST | rollback, reconciliação, foreign keys |
| container E2E | clone real e nove asserts SQL | UAST, rerun idempotente e falha controlada |
| compatibilidade | CI em stable | MSRV real, PostgreSQL suportado e upgrades |
| segurança | CodeQL | auth/TLS, SQL authorization e secret handling |

## Critérios para produção

O projeto deve ser considerado pronto para produção somente depois de:

1. corrigir consistência transacional e reconciliação;
2. alinhar toolchain e automatizar upgrade de schema/cache;
3. adicionar TLS e política de autorização ou remover exposição direta do pgwire;
4. publicar métricas, health/readiness e limites de recursos;
5. validar backup/restore e exclusão de dados;
6. medir capacidade com datasets representativos.
