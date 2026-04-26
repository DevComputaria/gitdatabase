mod handler;

pub use handler::GitbaseServerFactory;

use anyhow::Result;
use pgwire::tokio::process_socket;
use std::sync::Arc;
use tokio::net::TcpListener;

/// Start the pgwire-compatible TCP server.
pub async fn serve(bind_addr: &str, factory: Arc<GitbaseServerFactory>) -> Result<()> {
    let listener = TcpListener::bind(bind_addr).await?;
    tracing::info!("pgwire server listening on {bind_addr}");

    loop {
        let (socket, addr) = listener.accept().await?;
        tracing::debug!("new connection from {addr}");
        let factory = factory.clone();
        tokio::spawn(async move {
            if let Err(e) = process_socket(socket, None, factory).await {
                tracing::error!("connection error from {addr}: {e}");
            }
        });
    }
}
