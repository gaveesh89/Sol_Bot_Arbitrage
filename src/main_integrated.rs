//! Integrated MEV Bot Main Entry Point
//! 
//! This module provides a streamlined entry point for the MEV bot that uses
//! the integration layer to coordinate all components.
//! 
//! Components:
//! - Pool Monitor: Real-time WebSocket monitoring of DEX pools
//! - Arbitrage Detector: Bellman-Ford based opportunity detection
//! - Transaction Builder: Multi-DEX swap transaction construction
//! - Transaction Sender: Multi-RPC concurrent submission
//! 
//! Usage:
//!   BOT_SIMULATION_MODE=true cargo run --release     # Simulation mode (safe)
//!   BOT_SIMULATION_MODE=false cargo run --release    # Live execution (requires funds)

use anyhow::{Context, Result};
use solana_sdk::signature::{read_keypair_file, Keypair};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod chain;
mod config;
mod data;
mod dex;
mod meteora;
mod reporting;
mod utils;

use chain::integration::MevBotOrchestrator;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with colored output
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("Failed to create EnvFilter");

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false)
                .with_ansi(true),
        )
        .init();

    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║         🚀 Solana MEV Arbitrage Bot - Integrated            ║");
    info!("╚══════════════════════════════════════════════════════════════╝");
    info!("");

    // Load configuration
    info!("📋 Loading configuration...");
    let config = Config::load().context("Failed to load configuration")?;
    
    // Log important settings
    info!("⚙️  Configuration:");
    info!("   Mode: {}", if config.bot.is_simulation_mode { 
        "🎭 SIMULATION (Safe)" 
    } else { 
        "⚡ LIVE EXECUTION (Real funds at risk!)" 
    });
    info!("   Min Profit: {} bps ({:.2}%)", 
        config.bot.min_profit_bps, 
        config.bot.min_profit_bps as f64 / 100.0
    );
    info!("   Max Slippage: {} bps ({:.2}%)", 
        config.bot.max_slippage_bps,
        config.bot.max_slippage_bps as f64 / 100.0
    );
    info!("   Max Hops: {}", config.routing.max_hops);
    info!("   RPC: {}", config.rpc.url);
    info!("   WebSocket: {}", config.rpc.ws_url);
    info!("");

    // Load keypair
    info!("🔑 Loading wallet keypair...");
    let keypair = load_keypair(&config)?;
    let wallet_address = keypair.pubkey();
    info!("   Address: {}", wallet_address);

    // Check balance
    let rpc_client = solana_client::nonblocking::rpc_client::RpcClient::new(config.rpc.url.clone());
    match rpc_client.get_balance(&wallet_address).await {
        Ok(balance) => {
            let balance_sol = balance as f64 / 1e9;
            info!("   Balance: {:.4} SOL", balance_sol);
            
            if balance_sol < config.wallet.min_balance_sol {
                error!("❌ Insufficient balance! Minimum: {:.2} SOL", config.wallet.min_balance_sol);
                if !config.bot.is_simulation_mode {
                    anyhow::bail!("Add more SOL to wallet for live execution");
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to check balance: {}", e);
            if !config.bot.is_simulation_mode {
                anyhow::bail!("Cannot verify wallet balance for live execution");
            }
        }
    }
    info!("");

    // Initialize orchestrator
    info!("🔧 Initializing MEV Bot Orchestrator...");
    let mut orchestrator = MevBotOrchestrator::new(config.clone(), Arc::new(keypair))
        .await
        .context("Failed to initialize orchestrator")?;
    info!("");

    // Display startup banner
    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║                   🎯 Bot Ready to Trade                      ║");
    info!("╚══════════════════════════════════════════════════════════════╝");
    info!("");
    info!("📡 Pool Monitor: Active (WebSocket subscriptions)");
    info!("🔍 Arbitrage Detector: Active (Bellman-Ford algorithm)");
    info!("⚡ Transaction Executor: Active (Multi-RPC submission)");
    info!("📊 Metrics Reporter: Active (60s interval)");
    info!("");
    
    if config.bot.is_simulation_mode {
        info!("🎭 SIMULATION MODE: All opportunities will be logged but not executed");
        info!("   Set BOT_SIMULATION_MODE=false to enable live execution");
    } else {
        info!("⚡ LIVE EXECUTION MODE: Real trades will be executed!");
        info!("   Press Ctrl+C to stop the bot");
    }
    info!("");

    // Run the bot
    match orchestrator.run().await {
        Ok(_) => {
            info!("✅ Bot shutdown complete");
            Ok(())
        }
        Err(e) => {
            error!("❌ Bot error: {}", e);
            Err(e)
        }
    }
}

/// Load keypair from file or environment variable
fn load_keypair(config: &Config) -> Result<Keypair> {
    if let Some(keypair_path) = &config.wallet.keypair_path {
        // Load from file
        read_keypair_file(keypair_path)
            .with_context(|| format!("Failed to read keypair from {}", keypair_path))
    } else if let Some(private_key) = &config.wallet.private_key {
        // Parse from base58 string
        let bytes = bs58::decode(private_key)
            .into_vec()
            .context("Failed to decode private key")?;
        Keypair::from_bytes(&bytes).context("Failed to parse keypair from bytes")
    } else {
        anyhow::bail!("No keypair configured. Set WALLET_KEYPAIR_PATH or WALLET_PRIVATE_KEY")
    }
}
