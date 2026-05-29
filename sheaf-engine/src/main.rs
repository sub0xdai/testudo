//! Sheaf Engine — entry point.
//!
//! Starts the gRPC server and tick ingestion pipeline.

// @anchor infra:sheaf:main
// @tags infra

use clap::Parser;
use sheaf_engine::config::{Cli, RuntimeConfig};
use sheaf_engine::service::SheafEngineService;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sheaf_engine=info".into()),
        )
        .init();

    // Parse CLI.
    let cli = Cli::parse();
    let config = RuntimeConfig::from_cli(&cli);

    tracing::info!(
        "Sheaf Engine v{} starting — {} venues, {} symbols",
        env!("CARGO_PKG_VERSION"),
        config.venues.len(),
        config.symbols.len(),
    );

    for venue in &config.venues {
        tracing::info!("  venue: {} @ {}", venue.name, venue.url);
    }
    for symbol in &config.symbols {
        tracing::info!("  symbol: {}", symbol);
    }

    // Build the gRPC service.
    let service = SheafEngineService::new(config);

    // Start the gRPC server.
    let addr = cli.grpc_listen.parse()?;
    tracing::info!("gRPC server listening on {}", addr);

    Server::builder()
        .add_service(service.into_server())
        .serve(addr)
        .await?;

    Ok(())
}
