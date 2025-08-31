//! Testudo Trading Platform - Imperium Crate
//! 
//! The Imperium crate serves as the command and control center for the platform,
//! handling the main application entry point, API server, and overall orchestration.

use anyhow::Result;
use clap::{Arg, Command};
use config::{Config, Environment};
use std::net::SocketAddr;
use tracing::{info, warn};

// Note: These modules will be created within the imperium crate
mod config;
mod error;
mod routes;
mod middleware;

use crate::config::Settings;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("👑 Initializing Imperium Command Center");

    // Parse command line arguments
    let matches = Command::new("imperium")
        .version("0.1.0")
        .author("Testudo Platform Team")
        .about("Main executable for the Testudo Trading Platform")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config/default.toml"),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Server port")
                .default_value("3000"),
        )
        .get_matches();

    // Load configuration
    let config_file = matches.get_one::<String>("config").unwrap();
    let settings = Settings::new(config_file)?;

    info!("📋 Configuration loaded from: {}", config_file);

    // Initialize database connections
    let database_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(settings.database.max_connections)
        .connect(&settings.database.url)
        .await?;

    info!("🗄️ Database connection established");

    // Run database migrations
    sqlx::migrate!("./migrations").run(&database_pool).await?;
    info!("📈 Database migrations completed");

    // Initialize Redis connection
    let redis_client = redis::Client::open(settings.redis.url.as_str())?;
    let redis_manager = redis::aio::ConnectionManager::new(redis_client).await?;
    info!("🗲 Redis connection established");

    // Build application router
    let app = routes::create_router(database_pool, redis_manager, &settings);

    // Determine server address
    let port: u16 = matches.get_one::<String>("port").unwrap().parse()?;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("🚀 Imperium ready to command on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
