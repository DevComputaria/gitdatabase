# GitDatabase

Bem-vindo à documentação do **GitDatabase**.

O projeto indexa repositórios Git em PostgreSQL para consultas analíticas, busca de código e exploração via protocolo PostgreSQL (pgwire).

## Visão geral

O fluxo principal do sistema é:

1. Descobrir repositórios Git e sincronizar metadados (`sync`)
2. Hidratar blobs textuais no cache (`hydrate-blobs`)
3. Indexar busca textual (`search-index`)
4. (Opcional) Gerar estrutura semântica UAST (`uast`)
5. Consultar via SQL/pgwire (`serve`)

## Entradas e saídas principais

- **Entrada**: diretórios com repositórios Git (working tree ou bare)
- **Persistência**: schema `gitbase` no PostgreSQL
- **Saída**:
	- Tabelas e views SQL para análises
	- Função `gitbase.search_code(pattern, lang)` para busca full-text
	- Endpoint pgwire para ferramentas compatíveis com Postgres

## Navegação da documentação

- [Arquitetura](./architecture.md): componentes, fluxo e responsabilidades dos crates
- [Banco de dados](./database.md): tabelas, índices, função de busca e views
- [Pilot](./pilot.md): execução ponta a ponta para validação rápida
- [Runbook](./runbook.md): operação diária, manutenção e troubleshooting
