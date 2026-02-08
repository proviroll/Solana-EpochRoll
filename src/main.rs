mod client;
mod models;
mod service;
mod utils;

use crate::client::SolanaClient;
use crate::service::SFDPService;
use anyhow::Result;
use config::Config;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    // Load Configuration
    let settings = match Config::builder()
        .add_source(config::File::with_name("config.toml").required(true))
        .add_source(config::Environment::with_prefix("APP"))
        .build()
    {
        Ok(s) => s,
        Err(e) => {
            error!("Configuration error: {}. Did you create 'config.toml' from 'config.toml.example'?", e);
            std::process::exit(1);
        }
    };

    let mode = settings
        .get_string("validator_mode")
        .unwrap_or_else(|_| "firedancer".to_string())
        .into();
    let rpc_type = settings
        .get_string("rpc_type")
        .unwrap_or_else(|_| "remote".to_string())
        .into();
    let rpc_url = settings
        .get_string("rpc_url")
        .unwrap_or_else(|_| "https://api.testnet.solana.com".to_string());
    let sfdp_url = "https://api.solana.org/api/community/v1/sfdp_required_versions?cluster=testnet"
        .to_string();
    let slack_webhook = settings.get_string("slack_webhook_url")?;
    let identity = settings
        .get_string("validator_identity")
        .unwrap_or_else(|_| "".to_string());
    let check_secs = settings.get_int("check_interval_seconds").unwrap_or(1800) as u64;
    let report_hours = settings.get_int("report_interval_hours").unwrap_or(8);

    // Clients
    let client = SolanaClient::new(rpc_url, sfdp_url);

    // Pre-flight check
    info!(
        "Performing pre-flight connectivity check to {}...",
        client.get_rpc_url()
    );
    match client.get_version().await {
        Ok(v) => info!("Connected! Node version: {}", v),
        Err(e) => {
            error!("CRITICAL: Could not connect to RPC: {}", e);
            anyhow::bail!("Startup failed: RPC unreachable");
        }
    }

    // Service
    let mut service = SFDPService::new(
        client,
        mode,
        rpc_type,
        slack_webhook,
        identity,
        None, // Vote account auto-discovered
        check_secs,
        report_hours,
    );

    service.start().await
}
