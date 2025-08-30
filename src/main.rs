//! Testudo Trading Platform
//! 
//! A disciplined crypto trading platform implementing Van Tharp position sizing
//! methodology with Roman military precision and systematic risk management.
//!
//! ## Architecture
//! 
//! The platform follows the OODA Loop (Observe, Orient, Decide, Act) pattern:
//! - **Observe**: Market data ingestion and real-time monitoring
//! - **Orient**: Van Tharp position sizing and risk assessment  
//! - **Decide**: Testudo Protocol rule enforcement and validation
//! - **Act**: Exchange order execution with confirmation
//!
//! ## Core Principles
//! 
//! - **Disciplina**: Mathematical precision without deviation
//! - **Formatio**: Systematic execution under all conditions
//! - **Prudentia**: Risk-aware decision making
//! - **Imperium**: Clear command structure and control

use anyhow::Result;
use clap::{Arg, Command};
use config::{Config, Environment};
use std::net::SocketAddr;
use tracing::{info, warn};

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

    info!("🏛️ Initializing Testudo Trading Platform");

    // Parse command line arguments
    let matches = Command::new("testudo")
        .version("0.1.0")
        .author("Testudo Platform Team")
        .about("Disciplined crypto trading platform with Van Tharp position sizing")
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

    // Initialize core components following Roman military structure
    info!("⚔️ Initializing Core Components:");
    
    // Disciplina - Risk calculation engine
    info!("  🛡️ Disciplina: Van Tharp risk calculation engine");
    
    // Formatio - OODA loop trading operations
    info!("  🏹 Formatio: OODA loop trading operations");
    
    // Prudentia - Exchange integrations
    info!("  🏛️ Prudentia: Exchange integration adapters");
    
    // Imperium - Command and control
    info!("  👑 Imperium: API server and command interface");

    // Build application router
    let app = routes::create_router(database_pool, redis_manager, &settings);

    // Determine server address
    let port: u16 = matches.get_one::<String>("port").unwrap().parse()?;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("🚀 Testudo Trading Platform starting on {}", addr);
    info!("📊 Ready to serve disciplined traders");

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}