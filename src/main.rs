mod client;
mod models;
mod service;

use crate::client::SolanaClient;
use crate::service::SFDPService;
use anyhow::Result;
use std::env;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    // Load .env
    dotenvy::dotenv().ok();

    // Configuration
    let mode = env::var("VALIDATOR_MODE")
        .unwrap_or_else(|_| "firedancer".to_string())
        .into();
    let rpc_type = env::var("RPC_TYPE")
        .unwrap_or_else(|_| "remote".to_string())
        .into();
    let rpc_url =
        env::var("RPC_URL").unwrap_or_else(|_| "https://api.testnet.solana.com".to_string());
    let sfdp_url = env::var("SFDP_API_URL").unwrap_or_else(|_| {
        "https://api.solana.org/api/community/v1/sfdp_required_versions?cluster=testnet".to_string()
    });
    let slack_webhook = env::var("SLACK_WEBHOOK_URL")?;
    let identity = env::var("VALIDATOR_IDENTITY").unwrap_or_else(|_| "Unknown".to_string());
    let vote_account = env::var("VOTE_ACCOUNT").ok();
    let check_secs = env::var("CHECK_INTERVAL_SECONDS")
        .unwrap_or_else(|_| "1800".to_string())
        .parse()?;
    let report_hours = env::var("REPORT_INTERVAL_HOURS")
        .unwrap_or_else(|_| "8".to_string())
        .parse()?;

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
        vote_account,
        check_secs,
        report_hours,
    );

    service.start().await
}
