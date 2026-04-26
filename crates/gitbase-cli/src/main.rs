use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "gitbase",
    about = "Git repository analytics via PostgreSQL wire protocol"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Start the pgwire server
    Serve {
        /// Bind address for the pgwire listener
        #[arg(long, env = "GITBASE_BIND_ADDR", default_value = "0.0.0.0:5433")]
        bind: String,

        /// Root directories containing Git repositories
        #[arg(
            long,
            env = "GITBASE_REPO_ROOTS",
            value_delimiter = ',',
            num_args = 1..,
            default_value = "./"
        )]
        repo_roots: Vec<String>,

        /// PostgreSQL connection string
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,

        /// Maximum database connections
        #[arg(long, env = "GITBASE_DB_MAX_CONNECTIONS", default_value_t = 10)]
        max_connections: u32,

        /// Pgwire authentication user
        #[arg(long, env = "GITBASE_PG_USER", default_value = "gitbase")]
        pg_user: String,

        /// Pgwire authentication password
        #[arg(long, env = "GITBASE_PG_PASSWORD", default_value = "gitbase")]
        pg_password: String,

        /// Maximum blob size to hydrate (bytes)
        #[arg(long, env = "GITBASE_BLOB_MAX_BYTES", default_value_t = 1_000_000)]
        blob_max_bytes: u64,
    },

    /// Sync Git metadata into PostgreSQL
    Sync {
        /// Root directories containing Git repositories
        #[arg(
            long,
            env = "GITBASE_REPO_ROOTS",
            value_delimiter = ',',
            num_args = 1..,
            default_value = "./"
        )]
        repo_roots: Vec<String>,

        /// PostgreSQL connection string
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,

        /// Maximum database connections
        #[arg(long, env = "GITBASE_DB_MAX_CONNECTIONS", default_value_t = 10)]
        max_connections: u32,
    },

    /// Check database health and migrations
    Health {
        /// PostgreSQL connection string
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,

        /// Maximum database connections
        #[arg(long, env = "GITBASE_DB_MAX_CONNECTIONS", default_value_t = 5)]
        max_connections: u32,
    },

    /// Build UAST cache and projections
    Uast {
        /// PostgreSQL connection string
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,

        /// Maximum database connections
        #[arg(long, env = "GITBASE_DB_MAX_CONNECTIONS", default_value_t = 5)]
        max_connections: u32,

        /// Limit number of blobs to process
        #[arg(long, env = "GITBASE_UAST_LIMIT")]
        limit: Option<i64>,
    },

    /// Build code search index
    SearchIndex {
        /// PostgreSQL connection string
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,

        /// Maximum database connections
        #[arg(long, env = "GITBASE_DB_MAX_CONNECTIONS", default_value_t = 5)]
        max_connections: u32,

        /// Limit number of blobs to process
        #[arg(long, env = "GITBASE_SEARCH_LIMIT")]
        limit: Option<i64>,
    },

    /// Hydrate missing blob contents
    HydrateBlobs {
        /// Root directories containing Git repositories
        #[arg(
            long,
            env = "GITBASE_REPO_ROOTS",
            value_delimiter = ',',
            num_args = 1..,
            default_value = "./"
        )]
        repo_roots: Vec<String>,

        /// PostgreSQL connection string
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,

        /// Maximum database connections
        #[arg(long, env = "GITBASE_DB_MAX_CONNECTIONS", default_value_t = 5)]
        max_connections: u32,

        /// Maximum blob size to hydrate (bytes)
        #[arg(long, env = "GITBASE_BLOB_MAX_BYTES", default_value_t = 1_000_000)]
        blob_max_bytes: u64,

        /// Limit number of blobs to process
        #[arg(long, env = "GITBASE_BLOB_HYDRATE_LIMIT")]
        limit: Option<i64>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            bind,
            repo_roots,
            database_url,
            max_connections,
            pg_user,
            pg_password,
            blob_max_bytes,
        } => {
            let pool = gitbase_db::connect(&database_url, max_connections).await?;
            gitbase_db::health_check(&pool).await?;
            tracing::info!("health check passed");

            let roots = repo_roots
                .iter()
                .map(|root| root.into())
                .collect::<Vec<_>>();
            let blob_config = gitbase_loader::BlobHydrationConfig {
                max_blob_bytes: blob_max_bytes,
            };

            let factory = Arc::new(gitbase_pgwire::GitbaseServerFactory::new(
                pool,
                roots,
                blob_config,
                pg_user,
                pg_password,
            ));
            gitbase_pgwire::serve(&bind, factory).await?;
        }
        Commands::Sync {
            repo_roots,
            database_url,
            max_connections,
        } => {
            let pool = gitbase_db::connect(&database_url, max_connections).await?;
            gitbase_db::health_check(&pool).await?;
            tracing::info!("health check passed");

            let roots = repo_roots
                .iter()
                .map(|root| root.into())
                .collect::<Vec<_>>();
            let report = gitbase_loader::sync_repositories(&pool, &roots).await?;
            tracing::info!(
                repositories = report.repositories,
                refs = report.refs,
                commits = report.commits,
                commit_parents = report.commit_parents,
                tree_entries = report.tree_entries,
                files = report.files,
                "sync completed"
            );
        }
        Commands::Health {
            database_url,
            max_connections,
        } => {
            let pool = gitbase_db::connect(&database_url, max_connections).await?;
            gitbase_db::health_check(&pool).await?;
            tracing::info!("health check passed");
        }
        Commands::Uast {
            database_url,
            max_connections,
            limit,
        } => {
            let pool = gitbase_db::connect(&database_url, max_connections).await?;
            gitbase_db::health_check(&pool).await?;
            tracing::info!("health check passed");

            let report = gitbase_loader::index_uast(
                &pool,
                &gitbase_loader::UastIndexConfig {
                    max_candidates: limit,
                },
            )
            .await?;

            tracing::info!(
                parsed = report.parsed,
                skipped_cached = report.skipped_cached,
                skipped_missing_content = report.skipped_missing_content,
                skipped_unsupported_language = report.skipped_unsupported_language,
                "uast indexing completed"
            );
        }
        Commands::SearchIndex {
            database_url,
            max_connections,
            limit,
        } => {
            let pool = gitbase_db::connect(&database_url, max_connections).await?;
            gitbase_db::health_check(&pool).await?;
            tracing::info!("health check passed");

            let report = gitbase_loader::index_search(
                &pool,
                &gitbase_loader::SearchIndexConfig {
                    max_candidates: limit,
                },
            )
            .await?;

            tracing::info!(
                indexed = report.indexed,
                skipped_missing_content = report.skipped_missing_content,
                skipped_non_utf8 = report.skipped_non_utf8,
                skipped_empty = report.skipped_empty,
                "search indexing completed"
            );
        }
        Commands::HydrateBlobs {
            repo_roots,
            database_url,
            max_connections,
            blob_max_bytes,
            limit,
        } => {
            let pool = gitbase_db::connect(&database_url, max_connections).await?;
            gitbase_db::health_check(&pool).await?;
            tracing::info!("health check passed");

            let roots = repo_roots
                .iter()
                .map(|root| root.into())
                .collect::<Vec<_>>();
            let blob_config = gitbase_loader::BlobHydrationConfig {
                max_blob_bytes: blob_max_bytes,
            };

            let report =
                gitbase_loader::hydrate_missing_blobs(&pool, &roots, &blob_config, limit).await?;

            tracing::info!(
                hydrated = report.hydrated,
                cached_hits = report.cached_hits,
                skipped_binary = report.skipped_binary,
                skipped_oversized = report.skipped_oversized,
                missing = report.missing,
                "blob hydration completed"
            );
        }
    }

    Ok(())
}
