mod chain;
mod config;
mod dex;
mod meteora;
mod utils;

use anyhow::{Context, Result};
use chain::{MarketDataFetcher, PriceMonitor, TokenFetcher};
use chain::token_fetch::DexType;
use config::Config;
use meteora::{MeteoraDAMMClient, MeteoraVaultClient};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use utils::transaction::{MultiRpcSender, TransactionBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    // ========================================================================
    // Step 1: Initialize tracing subscriber with EnvFilter
    // ========================================================================
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("Failed to create EnvFilter");

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer()
            .with_target(false)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
        )
        .init();

    info!("🚀 Starting Solana MEV Bot...");
    info!("📅 Date: November 13, 2025");

    // ========================================================================
    // Step 2: Load configuration using Config::load()
    // ========================================================================
    let config = Config::load().context("Failed to load configuration")?;
    info!("✅ Configuration loaded successfully");
    debug!("Bot config: min_profit={} bps, max_slippage={} bps", 
        config.bot.min_profit_bps, config.bot.max_slippage_bps);
    debug!("Routing config: max_hops={}, multi_hop={}", 
        config.routing.max_hops, config.routing.enable_multi_hop);
    debug!("Loaded {} mint configurations", config.mints.len());

    // ========================================================================
    // Step 3: Initialize Keypair and derive wallet address
    // ========================================================================
    let payer = load_keypair(&config.wallet)?;
    let wallet_address = payer.pubkey();
    let payer_arc = Arc::new(payer);
    info!("✅ Wallet loaded: {}", wallet_address);
    info!("   Keypair type: {}", if config.wallet.keypair_path.is_some() { "File" } else { "Environment" });

    // ========================================================================
    // Step 4: Initialize RpcClient wrapped in Arc
    // ========================================================================
    let rpc_client = Arc::new(RpcClient::new(config.rpc.url.clone()));
    info!("✅ RPC client initialized");
    info!("   Primary RPC: {}", config.rpc.url);
    info!("   Backup RPCs: {}", config.rpc.backup_urls.len());
    info!("   Commitment: {}", config.rpc.commitment_level);

    // Check wallet balance
    match rpc_client.get_balance(&wallet_address).await {
        Ok(balance) => {
            let balance_sol = balance as f64 / 1e9;
            info!("💰 Wallet balance: {:.4} SOL ({} lamports)", balance_sol, balance);
            
            if balance_sol < config.wallet.min_balance_sol {
                warn!("⚠️  Low wallet balance! Current: {:.4} SOL, Minimum: {:.2} SOL", 
                    balance_sol, config.wallet.min_balance_sol);
                warn!("   Consider adding more SOL for transaction fees");
            }
        }
        Err(e) => {
            error!("❌ Failed to check wallet balance: {}", e);
            warn!("   Continuing anyway, but transactions may fail due to insufficient balance");
        }
    }

    // ========================================================================
    // Step 5: Initialize core components
    // ========================================================================
    
    // Initialize TokenFetcher with caching
    let token_fetcher = Arc::new(TokenFetcher::new(
        Arc::clone(&rpc_client),
        Duration::from_secs(config.cache.ttl_seconds),
        config.cache.max_size,
        config.bot.max_retries,
    ));
    info!("✅ Token fetcher initialized");
    info!("   Cache TTL: {}s, Max size: {}", config.cache.ttl_seconds, config.cache.max_size);
    info!("   Pool cache: {}, Account cache: {}", 
        config.cache.enable_pool_cache, config.cache.enable_account_cache);

    // Initialize MarketDataFetcher
    let market_data_fetcher = Arc::new(MarketDataFetcher::new(
        Arc::clone(&token_fetcher),
        Arc::clone(&rpc_client),
        config.bot.min_profit_bps,
        config.bot.max_slippage_bps,
    ));
    info!("✅ Market data fetcher initialized");
    info!("   Min profit: {} bps ({}%)", config.bot.min_profit_bps, config.bot.min_profit_bps as f64 / 100.0);
    info!("   Max slippage: {} bps ({}%)", config.bot.max_slippage_bps, config.bot.max_slippage_bps as f64 / 100.0);

    // Initialize Meteora clients
    let meteora_damm_client = Arc::new(MeteoraDAMMClient::new(
        Arc::clone(&rpc_client),
        config.dex.meteora_damm_program_id,
        Arc::clone(&payer_arc),
    ));
    info!("✅ Meteora DAMM client initialized");
    debug!("   DAMM Program ID: {}", config.dex.meteora_damm_program_id);

    let meteora_vault_client = Arc::new(MeteoraVaultClient::new(
        Arc::clone(&rpc_client),
        config.dex.meteora_vault_program_id,
        Arc::clone(&payer_arc),
    ));
    info!("✅ Meteora Vault client initialized");
    debug!("   Vault Program ID: {}", config.dex.meteora_vault_program_id);

    // Initialize multi-RPC sender for transaction spamming
    let mut rpc_urls = vec![config.rpc.url.clone()];
    rpc_urls.extend(config.rpc.backup_urls.clone());
    let _multi_rpc_sender = MultiRpcSender::new(rpc_urls.clone());
    info!("✅ Multi-RPC sender initialized");
    if config.spam.enabled {
        info!("   Spam mode: ENABLED ({} submissions, {}ms delay)", 
            config.spam.num_submissions, config.spam.delay_ms);
        info!("   Using {} RPC endpoints", rpc_urls.len());
    } else {
        info!("   Spam mode: DISABLED");
    }

    // ========================================================================
    // Step 6-8: Process all configured mints and their pools
    // ========================================================================
    info!("📊 Processing mint configurations and pool data...");
    
    let mut all_pools_to_monitor: Vec<(Pubkey, DexType)> = Vec::new();
    
    // Loop through all configured mints
    for (idx, mint_config) in config.mints.iter().enumerate() {
        info!("🪙 Processing mint {}/{}: {} ({})", 
            idx + 1, config.mints.len(), mint_config.symbol, mint_config.address);
        debug!("   Decimals: {}, Is quote: {}, Pools: {}", 
            mint_config.decimals, mint_config.is_quote, mint_config.pools.len());
        
        if mint_config.pools.is_empty() {
            warn!("   ⚠️  No pools configured for {}", mint_config.symbol);
            continue;
        }

        // For each pool associated with this mint
        for pool_pubkey in &mint_config.pools {
            // TODO: Determine DEX type from pool address or configuration
            // For now, we'll use a placeholder - you should implement proper DEX detection
            let dex_type = DexType::Raydium; // Placeholder
            
            all_pools_to_monitor.push((*pool_pubkey, dex_type.clone()));
            debug!("   Added pool: {} ({:?})", pool_pubkey, dex_type);
        }
    }

    if all_pools_to_monitor.is_empty() {
        warn!("⚠️  No pools configured for monitoring!");
        warn!("📝 Please configure pools in your .env file using MINT_X_POOLS");
        info!("💡 Example configuration:");
        info!("   MINT_1_ADDRESS=So11111111111111111111111111111111111111112");
        info!("   MINT_1_SYMBOL=SOL");
        info!("   MINT_1_POOLS=POOL_ADDRESS_1,POOL_ADDRESS_2");
        
        // Keep bot running in demo mode
        info!("🛑 Running in demo mode (no pools configured)");
        tokio::signal::ctrl_c().await?;
        info!("👋 Shutting down...");
        return Ok(());
    }

    // Step 7: Initialize pool data for all collected pools
    info!("📥 Fetching initial pool data for {} pools...", all_pools_to_monitor.len());
    let initialized_pools = token_fetcher
        .initialize_pool_data(&all_pools_to_monitor)
        .await?;
    info!("✅ Pool data initialized: {}/{} pools loaded successfully", 
        initialized_pools.len(), all_pools_to_monitor.len());

    // Step 8: Fetch prices and calculate initial arbitrage opportunities
    info!("💹 Fetching initial prices and checking for arbitrage opportunities...");
    match market_data_fetcher.calculate_arbitrage_opportunities(&all_pools_to_monitor).await {
        Ok(opportunities) => {
            if opportunities.is_empty() {
                info!("   No arbitrage opportunities found at startup");
            } else {
                info!("   🎯 Found {} arbitrage opportunities at startup!", opportunities.len());
                
                // Step 9: Print results and potential opportunities
                for (i, opp) in opportunities.iter().enumerate() {
                    info!("   Opportunity #{}: {} -> {}", i + 1, opp.token_a_mint, opp.token_b_mint);
                    info!("      Buy:  {:?} @ {:.8}", opp.buy_dex, opp.buy_price);
                    info!("      Sell: {:?} @ {:.8}", opp.sell_dex, opp.sell_price);
                    info!("      Gross Profit: {} bps ({:.2}%)", opp.gross_profit_bps, opp.gross_profit_bps as f64 / 100.0);
                    info!("      Net Profit: {} bps ({:.2}%)", opp.net_profit_bps, opp.net_profit_bps as f64 / 100.0);
                    info!("      Total Fees: {} bps, Slippage: {} bps", opp.total_fees_bps, opp.estimated_slippage_bps);
                    info!("      Recommended amount: {} lamports", opp.recommended_amount);
                    info!("      Risk: {:?}", opp.execution_risk);
                }
            }
        }
        Err(e) => {
            warn!("   Failed to calculate initial arbitrage opportunities: {}", e);
        }
    }

    // ========================================================================
    // Step 10: Start continuous price monitoring (PriceMonitor)
    // ========================================================================
    let price_monitor = PriceMonitor::new(
        Arc::clone(&market_data_fetcher),
        Duration::from_millis(config.monitoring.price_check_interval_ms),
        config.monitoring.price_change_threshold_bps, // Threshold for triggering arbitrage calculation
    );

    info!("🎯 Bot initialization complete!");
    info!("📈 Starting continuous price monitoring...");
    info!("");
    info!("⚙️  Active Configuration Summary:");
    info!("   ├─ Strategy: Arbitrage={}, Sandwich={}", 
        config.bot.enable_arbitrage, config.bot.enable_sandwich);
    info!("   ├─ Profit threshold: {} bps ({:.2}%)", 
        config.bot.min_profit_bps, config.bot.min_profit_bps as f64 / 100.0);
    info!("   ├─ Max slippage: {} bps ({:.2}%)", 
        config.bot.max_slippage_bps, config.bot.max_slippage_bps as f64 / 100.0);
    info!("   ├─ Price check interval: {}ms", config.monitoring.price_check_interval_ms);
    info!("   ├─ Max retries: {}", config.bot.max_retries);
    info!("   ├─ Compute unit limit: {}", config.execution.compute_unit_limit);
    info!("   ├─ Compute unit price: {} micro-lamports", config.execution.compute_unit_price);
    info!("   ├─ Versioned transactions: {}", config.execution.use_versioned_transactions);
    info!("   ├─ Simulate before send: {}", config.execution.simulate_before_send);
    info!("   ├─ Transaction spam: {} ({}x submissions)", 
        config.spam.enabled, config.spam.num_submissions);
    info!("   ├─ Flash loans: {}", config.flashloan.enabled);
    info!("   ├─ Multi-hop routing: {} (max {} hops)", 
        config.routing.enable_multi_hop, config.routing.max_hops);
    info!("   └─ Monitoring {} pools across {} mints", 
        all_pools_to_monitor.len(), config.mints.len());
    info!("");
    
    if config.bot.enable_arbitrage {
        info!("✅ Arbitrage mode ENABLED");
    } else {
        warn!("⚠️  Arbitrage mode DISABLED - no trades will be executed");
    }
    
    if config.flashloan.enabled {
        info!("💡 Flash loan integration ENABLED (provider: {})", config.flashloan.provider);
    }
    
    info!("");
    info!("🔄 Monitoring loop starting... Press Ctrl+C to stop");
    info!("");

    // Start monitoring loop
    // NOTE: Arbitrage execution is implemented but should be tested thoroughly
    // on devnet before enabling on mainnet!
    price_monitor
        .start_monitoring(all_pools_to_monitor)
        .await
        .context("Price monitoring failed")?;

    Ok(())
}

/// Load wallet keypair from file or environment variable
fn load_keypair(wallet_config: &config::WalletConfig) -> Result<Keypair> {
    if let Some(ref keypair_path) = wallet_config.keypair_path {
        info!("Loading keypair from file: {}", keypair_path);
        read_keypair_file(keypair_path)
            .map_err(|e| anyhow::anyhow!("Failed to read keypair file: {}", e))
    } else if let Some(ref private_key) = wallet_config.private_key {
        info!("Loading keypair from environment variable");
        let decoded = bs58::decode(private_key)
            .into_vec()
            .context("Failed to decode base58 private key")?;
        Keypair::from_bytes(&decoded).context("Failed to create keypair from bytes")
    } else {
        Err(anyhow::anyhow!(
            "No wallet configuration found. Set WALLET_KEYPAIR_PATH or WALLET_PRIVATE_KEY"
        ))
    }
}

/// Example function for executing arbitrage (currently not called)
#[allow(dead_code)]
async fn execute_arbitrage(
    opportunity: &chain::token_price::ArbitrageOpportunity,
    rpc_client: Arc<RpcClient>,
    payer: Arc<Keypair>,
    config: &Config,
) -> Result<String> {
    info!("🎯 Executing arbitrage opportunity:");
    info!("   Buy on {:?} @ {}", opportunity.buy_dex, opportunity.buy_price);
    info!("   Sell on {:?} @ {}", opportunity.sell_dex, opportunity.sell_price);
    info!("   Expected gross profit: {} bps", opportunity.gross_profit_bps);
    info!("   Expected net profit: {} bps", opportunity.net_profit_bps);

    // Build transaction
    let mut tx_builder = TransactionBuilder::new(payer.pubkey());
    tx_builder
        .set_compute_unit_limit(config.execution.compute_unit_limit)
        .set_compute_unit_price(config.execution.compute_unit_price);

    // TODO: Add actual swap instructions here
    // Example:
    // 1. Buy instruction (buy_dex)
    // 2. Sell instruction (sell_dex)
    
    // Simulate transaction
    tx_builder.simulate(&rpc_client, &payer).await?;

    // Build final transaction
    let transaction = tx_builder.build(&rpc_client, &payer).await?;

    // Send to multiple RPCs for higher inclusion probability
    let multi_rpc_sender = MultiRpcSender::new(vec![config.rpc.url.clone()]);
    let signature = multi_rpc_sender
        .send_and_get_first_success(&transaction)
        .await?;

    info!("✅ Arbitrage executed successfully: {}", signature);
    Ok(signature.to_string())
}
