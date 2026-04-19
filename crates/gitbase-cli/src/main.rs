use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "gitbase", about = "Git repository analytics via PostgreSQL wire protocol")]
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

        /// PostgreSQL connection string
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,

        /// Maximum database connections
        #[arg(long, env = "GITBASE_DB_MAX_CONNECTIONS", default_value_t = 10)]
        max_connections: u32,
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
            database_url,
            max_connections,
        } => {
            let pool = gitbase_db::connect(&database_url, max_connections).await?;
            gitbase_db::health_check(&pool).await?;
            tracing::info!("health check passed");

            let factory = Arc::new(gitbase_pgwire::GitbaseServerFactory::new(pool));
            gitbase_pgwire::serve(&bind, factory).await?;
        }
    }

    Ok(())
}
