# Pipelines, consistência e recuperação

Esta página detalha o ciclo de vida dos dados desde o repositório Git até as
interfaces SQL. Os nomes das operações correspondem às funções e comandos atuais.

## Pipeline completo

```text
discover_repositories
        │
        ▼
collect_refs + collect_commits
        │
        ▼
repositories / refs / commits / trees / files
        │
        ▼
hydrate_missing_blobs ──▶ blobs.content
        │
        ├──▶ index_search ──▶ code_index ──▶ search_code(...)
        │
        └──▶ index_uast ────▶ uast_cache ──▶ functions / imports
```

Cada estágio é executado separadamente. Não existe um coordenador persistente de
jobs nem uma tabela de checkpoints do pipeline.

## 1. Descoberta de repositórios

`gitbase-git::discover_repositories` aceita um ou mais roots.

1. Se o root for um working tree ou bare repo, ele é usado diretamente.
2. Caso contrário, `walkdir` procura diretórios `.git` e estruturas bare.
3. Symlinks não são seguidos.
4. O ID é calculado com SHA-1 do path textual.
5. O nome é o último componente do path.

Consequência: dois paths diferentes para o mesmo remoto são entidades distintas,
e uma mudança de path não é reconhecida como rename.

## 2. Sincronização de metadados

`sync_repositories` processa os repositórios sequencialmente.

```text
para cada repositório
  upsert repository
  coletar e upsert refs
  caminhar commits alcançáveis a partir das refs
  carregar hashes de commits já existentes
  para cada commit novo
    upsert commit e pais
    upsert todas as tree_entries
    upsert todos os files
```

### Idempotência

- repositórios e refs usam chaves naturais compostas para `UPSERT`;
- commits já existentes são ignorados por completo;
- repetir um sync sem novos commits tende a gerar apenas upserts de repositório e
  refs;
- dados parcialmente gravados para um commit já existente não são reparados por
  uma repetição normal, pois o commit causa o `continue` do snapshot inteiro.

### Consistência atual

Cada statement usa uma conexão do pool e faz commit individual. Não há transação
por repositório. Uma interrupção pode deixar commit sem todos os pais, trees ou
files. Também não há fase de reconciliação para apagar refs removidas ou objetos
que ficaram inalcançáveis após rebase/force-push.

## 3. Hidratação de blobs

O sync armazena referências a hashes, mas não lê todo conteúdo imediatamente.
`hydrate_missing_blobs` seleciona hashes de arquivos sem `blobs.content` e chama
`hydrate_blobs`.

Para cada hash:

1. um cache com conteúdo é reutilizado e recebe novo `last_used_at`;
2. o banco informa em quais repositórios o hash aparece;
3. paths fora dos `repo_roots` permitidos são descartados;
4. o objeto é lido do primeiro repositório capaz de fornecê-lo;
5. binários e blobs acima do limite são registrados sem conteúdo;
6. texto aceito é persistido como `bytea`.

O cache é global por SHA-1 do blob, portanto conteúdo idêntico entre repositórios
é armazenado uma vez. Não existe política automática de expiração ou garbage
collection, apesar de `last_used_at` permitir implementá-la.

## 4. Índice de busca

`fetch_search_candidates` seleciona blobs textuais hidratados que ainda não estão
em `code_index`. O conteúdo UTF-8 não vazio vira um `tsvector` com configuração
`simple`.

A linguagem é obtida primeiro de `files.language`; quando ela está ausente, há
fallback por extensão apenas para Go e Rust usando o detector de UAST. Assim, o
filtro `lang` não cobre as demais linguagens presentes nos repositórios.

`gitbase.search_code(pattern, lang)`:

- transforma o padrão com `websearch_to_tsquery('simple', ...)`;
- filtra por linguagem somente quando `lang` é informado;
- calcula score com `ts_rank_cd`;
- faz join com todas as ocorrências do blob em `files`.

Por isso, o mesmo blob pode gerar várias linhas quando aparece em vários commits.

## 5. UAST e projeções

Os candidatos são blobs hidratados associados a algum path. O path determina a
linguagem:

- `.go` → Go;
- `.rs` → Rust;
- outras extensões → não suportadas.

O tree-sitter produz um documento compacto com funções e imports, armazenado em
`uast_cache`. As tabelas `uast_functions` e `uast_imports` facilitam consultas
relacionais; as views acrescentam repositório, commit e path via `files`.

O cache é identificado somente pelo hash do blob. Alterações na versão do parser
ou no formato da projeção não invalidam automaticamente documentos antigos.

## 6. Consulta pgwire e hidratação sob demanda

O handler classifica uma consulta como leitura somente quando o texto, após
trim, começa com `SELECT`.

- `SELECT`: procura literais de 40 caracteres próximos a `blob_hash`, hidrata até
  50 hashes, executa a consulta e materializa todas as linhas antes da resposta;
- demais consultas: executa o SQL e retorna uma tag `OK` com linhas afetadas.

A extração de hashes é lexical, não usa parser SQL. CTEs iniciadas com `WITH`,
prepared statements e protocolo estendido não têm tratamento especializado.

## Matriz de reexecução

| Estágio | Seguro repetir | Unidade incremental | Reparo de dados parciais |
| --- | --- | --- | --- |
| `health` | sim | migration | SQLx controla versões |
| `sync` | parcial | commit novo | não para commit já existente |
| `hydrate-blobs` | sim | blob sem conteúdo | sim, se a origem estiver acessível |
| `search-index` | sim | blob sem índice | exige truncar para reindexar conteúdo existente |
| `uast` | sim | blob sem cache | exige truncar para nova versão do parser |

## Falhas e recuperação

| Falha | Estado provável | Recuperação atual |
| --- | --- | --- |
| repositório ilegível | sync interrompido | corrigir permissão/path e repetir |
| PostgreSQL indisponível | comando falha | restaurar conexão e repetir |
| interrupção durante sync | metadados parciais possíveis | inspeção manual; commit existente pode exigir limpeza |
| blob ausente no objeto Git | contador `missing` | restaurar objetos/fetch e repetir hidratação |
| conteúdo acima do limite | blob sem conteúdo | aumentar limite e repetir após revisar cache |
| parser UAST falha | comando interrompido | corrigir parser/conteúdo e reindexar |
| índice desatualizado | resultado antigo | truncar índice/cache e executar novamente |

## Métricas recomendadas

Até existir telemetria nativa, estas métricas podem ser derivadas do banco:

- duração e exit code de cada comando;
- commits novos por sync;
- razão entre blobs hidratados e hashes distintos em `files`;
- razão entre `code_index` e blobs textuais hidratados;
- cobertura UAST por linguagem;
- idade de `cached_at`, `last_used_at` e `indexed_at`;
- refs do banco que não existem mais no repositório de origem;
- commits sem files/tree entries como indicador de carga parcial.
