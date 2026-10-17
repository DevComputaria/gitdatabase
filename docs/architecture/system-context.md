# Contexto e componentes do sistema

Este documento descreve as fronteiras do GitDatabase, os processos executáveis e
as dependências entre os crates. Ele representa o código disponível no branch
principal e não pressupõe serviços que ainda estejam no roadmap.

## Objetivo arquitetural

O GitDatabase transforma objetos Git locais em um modelo relacional consultável.
O PostgreSQL é, ao mesmo tempo, armazenamento persistente, mecanismo de busca
textual e backend das consultas expostas por pgwire.

O sistema foi desenhado para estes casos de uso:

- inventariar repositórios, refs, commits, árvores e arquivos;
- consultar histórico e conteúdo com SQL;
- compartilhar conteúdo idêntico por `blob_hash`;
- buscar termos em blobs textuais;
- projetar funções e imports de Go e Rust;
- permitir acesso por clientes compatíveis com o protocolo PostgreSQL.

Não fazem parte da implementação atual: hospedar repositórios Git, clonar ou
atualizar remotos automaticamente, executar código indexado, oferecer uma API
HTTP ou substituir o PostgreSQL como fonte de consulta.

## Visão de contexto

```text
┌──────────────────────┐       filesystem       ┌──────────────────────┐
│ Repositórios Git     │ ─────────────────────▶ │ GitDatabase          │
│ working tree / bare  │                        │ CLI + loaders        │
└──────────────────────┘                        └──────────┬───────────┘
                                                        │ SQL/sqlx
                                                        ▼
┌──────────────────────┐       pgwire/SQL        ┌──────────────────────┐
│ Clientes SQL         │ ◀────────────────────── │ PostgreSQL           │
│ psql, BI, scripts    │   via GitDatabase       │ schema gitbase       │
└──────────────────────┘   ou acesso direto      └──────────────────────┘
```

### Atores e responsabilidades

| Ator | Fornece | Consome |
| --- | --- | --- |
| Operador | paths, credenciais e limites de processamento | logs, exit code e relatórios |
| Repositório Git | refs, commits, trees e blobs | operações de leitura local |
| PostgreSQL | persistência, migrations, índices e execução SQL | conexões autenticadas |
| Cliente SQL | consultas e credenciais pgwire | linhas e tags de execução |
| CI | checkout, Docker e rede para o repositório de teste | relatório Markdown e status |

## Visão de containers de runtime

O termo *container* nesta seção representa uma unidade executável, não
necessariamente um container Docker.

### CLI `gitbase`

Processo batch que seleciona um dos comandos `health`, `sync`, `hydrate-blobs`,
`search-index` ou `uast`. Cada execução cria um pool `sqlx`, aplica migrations e
encerra ao concluir. O comando `serve` mantém um listener pgwire ativo.

### PostgreSQL

Componente stateful. As migrations são compiladas no binário por
`sqlx::migrate!` e executadas durante toda conexão criada por `gitbase-db`. Os
objetos de domínio ficam no schema `gitbase`; o controle de migrations do SQLx
fica na tabela `_sqlx_migrations`.

### Filesystem Git

Fonte de verdade dos objetos Git. O banco guarda o path absoluto descoberto e a
hidratação posterior depende de esse path continuar acessível. Mover ou apagar o
repositório pode impedir a leitura de blobs ainda não hidratados.

### Gateway pgwire

Listener TCP que implementa autenticação por senha em texto claro no protocolo e
o protocolo de consulta simples. As consultas são encaminhadas ao PostgreSQL.
Antes de um `SELECT`, hashes literais associados a `blob_hash` podem disparar
hidratação sob demanda, limitada a 50 hashes por consulta.

## Componentes Rust

| Crate | Responsabilidade | Dependências internas |
| --- | --- | --- |
| `gitbase-cli` | parsing de argumentos, configuração e orquestração | db, loader, pgwire |
| `gitbase-git` | descoberta e leitura de objetos Git com `gix` | nenhuma |
| `gitbase-db` | conexão, migrations e operações SQL | nenhuma |
| `gitbase-loader` | coordenação dos pipelines de ingestão e índices | git, db, uast |
| `gitbase-uast` | detecção Go/Rust e projeções tree-sitter | nenhuma |
| `gitbase-pgwire` | autenticação, protocolo simples e proxy SQL | loader, db |

```text
gitbase-cli
├── gitbase-loader
│   ├── gitbase-git
│   ├── gitbase-db
│   └── gitbase-uast
├── gitbase-db
└── gitbase-pgwire
    ├── gitbase-loader
    └── gitbase-db
```

## Modos de execução

### Batch explícito

É o modo recomendado para cargas previsíveis. O operador executa cada estágio e
controla limites por comando. Uma falha retorna exit code diferente de zero e o
estágio pode ser repetido porque as escritas usam `UPSERT` ou verificam o cache.

### Serviço pgwire

É adequado para clientes SQL que não acessam diretamente o PostgreSQL. O gateway
não isola o banco: consultas que não começam com `SELECT` são executadas com as
permissões da `DATABASE_URL`, inclusive DDL e DML.

### Integração em containers

O Compose de integração cria PostgreSQL efêmero e um executor contendo a CLI,
`git` e `psql`. O executor clona o repositório de referência, percorre todo o
pipeline e produz um relatório que a GitHub Action envia ao Job Summary.

## Fronteiras de confiança

1. **Operador → CLI:** paths e URLs devem ser considerados entrada não confiável.
2. **Repositório → parsers:** conteúdo de blobs não é confiável; há limite de
   tamanho, classificação binária e validação UTF-8 antes dos índices.
3. **Cliente → pgwire:** SQL é encaminhado ao banco sem política de autorização
   por comando. A credencial do backend deve aplicar privilégio mínimo.
4. **GitDatabase → PostgreSQL:** a conexão deve permanecer em rede privada e com
   TLS quando atravessar uma fronteira de host.

## Topologias suportadas

| Topologia | Uso | Observações |
| --- | --- | --- |
| CLI e PostgreSQL locais | desenvolvimento | compose atual publica PostgreSQL em `5433` |
| CLI em job + PostgreSQL gerenciado | cargas agendadas | repositórios precisam estar montados no job |
| pgwire + PostgreSQL na mesma rede | consultas compartilhadas | não expor pgwire sem proteção de transporte |
| Compose efêmero de integração | CI | banco usa `tmpfs` e é descartado ao final |

## Restrições técnicas atuais

- o ID do repositório é SHA-1 do path; mover o diretório muda sua identidade;
- a sincronização não remove refs ou commits que deixaram de ser alcançáveis;
- não há transação envolvendo todo o sync de um repositório;
- as tabelas de metadados não possuem foreign keys;
- `files.language`, `files.size` e a classificação binária inicial não são
  preenchidos a partir do tree walk;
- UAST suporta somente Go e Rust;
- pgwire implementa apenas simple query e materializa todas as linhas em memória;
- o toolchain declarado como Rust 1.80 diverge do `Cargo.lock`, que atualmente
  contém dependências que exigem Rust 1.89.

As correções propostas estão organizadas no [plano de funcionalidades](../roadmap.md).
