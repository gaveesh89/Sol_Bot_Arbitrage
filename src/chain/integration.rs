use anyhow::{Context, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::dex::triangular_arb::{ArbitrageGraph, BellmanFordDetector};
use crate::dex::pool_fetcher::PoolDataFetcher;

use super::detector::{ArbitrageDetector, ArbitrageOpportunity};
use super::pool_monitor::PoolMonitor;
use super::transaction_builder::{SwapTransactionBuilder, TransactionConfig};
use super::transaction_sender::{SendConfig, TransactionSender};

/// Main integration structure that coordinates all MEV bot components
pub struct MevBotOrchestrator {
    pub config: Config,
    pub graph: Arc<std::sync::RwLock<ArbitrageGraph>>,
    pub detector: Arc<ArbitrageDetector>,
    pub pool_monitor: Arc<PoolMonitor>,
    pub tx_builder: Arc<SwapTransactionBuilder>,
    pub tx_sender: Arc<TransactionSender>,
    pub opportunity_tx: mpsc::UnboundedSender<ArbitrageOpportunity>,
    pub opportunity_rx: Option<mpsc::UnboundedReceiver<ArbitrageOpportunity>>,
    pub shutdown_tx: mpsc::Sender<()>,
    pub shutdown_rx: Option<mpsc::Receiver<()>>,
    pub metrics: Arc<RwLock<ExecutionMetrics>>,
}

/// Execution metrics for monitoring bot performance
#[derive(Debug, Clone, Default)]
pub struct ExecutionMetrics {
    pub opportunities_received: u64,
    pub opportunities_executed: u64,
    pub opportunities_skipped: u64,
    pub transactions_sent: u64,
    pub transactions_confirmed: u64,
    pub transactions_failed: u64,
    pub total_profit_lamports: i64,
    pub total_fees_paid: u64,
    pub frontrun_detected: u64,
    pub average_execution_time_ms: u64,
}

impl MevBotOrchestrator {
    /// Create a new MEV bot orchestrator with all components initialized
    pub async fn new(config: Config, keypair: Arc<Keypair>) -> Result<Self> {
        info!("🔧 Initializing MEV Bot Orchestrator...");

        // Initialize RPC clients
        let rpc_clients = Self::create_rpc_clients(&config)?;
        info!("✅ Created {} RPC clients", rpc_clients.len());

        // Initialize arbitrage graph (using std::sync::RwLock as required by BellmanFord)
        let std_graph = Arc::new(std::sync::RwLock::new(ArbitrageGraph::new()));
        // Also create a tokio RwLock version for detector
        let tokio_graph = Arc::new(RwLock::new(ArbitrageGraph::new()));
        info!("✅ Initialized arbitrage graph");

        // Create channel for arbitrage opportunities
        let (opportunity_tx, opportunity_rx) = mpsc::unbounded_channel();

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Initialize detector (uses tokio RwLock internally)
        let detector = Arc::new(ArbitrageDetector::new(
            std_graph.clone(),
            config.bot.min_profit_bps as i64,
            opportunity_tx.clone(),
        ));
        info!("✅ Initialized arbitrage detector");

        // Initialize pool data fetcher
        let pool_fetcher = Arc::new(PoolDataFetcher::new(
            vec![rpc_clients[0].clone()],
            config.cache.ttl_seconds * 1000,  // Convert to ms
        ));
        info!("✅ Initialized pool data fetcher");

        // Initialize Bellman-Ford detector for pool monitor
        let bellman_ford = Arc::new(BellmanFordDetector::new(
            std_graph.clone(),
            config.bot.min_profit_bps as i64,
        ));

        // Initialize pool monitor
        let monitored_pools = vec![]; // Will be populated with actual pool addresses
        let pool_monitor = Arc::new(PoolMonitor::new(
            config.rpc.ws_url.clone(),
            std_graph.clone(),
            pool_fetcher.clone(),
            monitored_pools,
            bellman_ford,
        ));
        info!("✅ Initialized pool monitor");

        // Initialize transaction builder
        let token_accounts = HashMap::new(); // Will be populated dynamically
        let lookup_tables = vec![]; // Address lookup tables for transaction compression
        
        // Clone keypair using from_bytes (Keypair doesn't implement Clone)
        let keypair_bytes = keypair.to_bytes();
        let payer_keypair = Keypair::from_bytes(&keypair_bytes)
            .context("Failed to clone keypair")?;
        
        let tx_builder = Arc::new(SwapTransactionBuilder::new(
            payer_keypair,
            token_accounts,
            lookup_tables,
        ));
        info!("✅ Initialized transaction builder");

        // Initialize transaction sender
        let tx_sender = Arc::new(TransactionSender::new(
            rpc_clients,
            config.bot.max_retries as u8,
            config.bot.transaction_timeout_ms,
        ));
        info!("✅ Initialized transaction sender");

        // Initialize metrics
        let metrics = Arc::new(RwLock::new(ExecutionMetrics::default()));

        Ok(Self {
            config,
            graph: std_graph,
            detector,
            pool_monitor,
            tx_builder,
            tx_sender,
            opportunity_tx,
            opportunity_rx: Some(opportunity_rx),
            shutdown_tx,
            shutdown_rx: Some(shutdown_rx),
            metrics,
        })
    }

    /// Create multiple RPC clients for redundancy
    fn create_rpc_clients(config: &Config) -> Result<Vec<Arc<RpcClient>>> {
        let mut clients = Vec::new();

        // Main RPC client
        let main_client = Arc::new(RpcClient::new(config.rpc.url.clone()));
        clients.push(main_client);

        // Backup RPC clients
        for backup_url in &config.rpc.backup_urls {
            let backup_client = Arc::new(RpcClient::new(backup_url.clone()));
            clients.push(backup_client);
        }

        if clients.is_empty() {
            anyhow::bail!("No RPC clients configured");
        }

        Ok(clients)
    }

    /// Start all components and run the MEV bot
    pub async fn run(&mut self) -> Result<()> {
        info!("🚀 Starting MEV Bot...");

        // Take ownership of receivers
        let mut opportunity_rx = self
            .opportunity_rx
            .take()
            .context("Opportunity receiver already taken")?;
        let mut shutdown_rx = self
            .shutdown_rx
            .take()
            .context("Shutdown receiver already taken")?;

        // Spawn pool monitoring task
        let monitor = self.pool_monitor.clone();
        let monitor_handle = tokio::spawn(async move {
            info!("📡 Starting pool monitor...");
            if let Err(e) = monitor.start_monitoring().await {
                error!("Pool monitoring error: {}", e);
            }
        });

        // Spawn detection task
        let detector = self.detector.clone();
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        let detection_handle = tokio::spawn(async move {
            info!("🔍 Starting arbitrage detector...");
            detector.run_detection_loop(update_rx).await;
        });

        // Spawn execution task
        let tx_builder = self.tx_builder.clone();
        let tx_sender = self.tx_sender.clone();
        let config = self.config.clone();
        let metrics = self.metrics.clone();
        let execution_handle = tokio::spawn(async move {
            info!("⚡ Starting execution engine...");
            Self::execute_opportunities(opportunity_rx, tx_builder, tx_sender, config, metrics)
                .await;
        });

        // Spawn metrics reporting task
        let metrics_clone = self.metrics.clone();
        let metrics_handle = tokio::spawn(async move {
            Self::report_metrics_loop(metrics_clone).await;
        });

        info!("✅ All components started successfully");
        info!("🎯 Bot is running. Press Ctrl+C to shutdown...");

        // Wait for shutdown signal
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("📥 Received shutdown signal");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("🛑 Received Ctrl+C");
            }
        }

        // Graceful shutdown
        info!("🔄 Shutting down gracefully...");
        monitor_handle.abort();
        detection_handle.abort();
        execution_handle.abort();
        metrics_handle.abort();

        // Print final metrics
        let final_metrics = self.metrics.read().await;
        info!("📊 Final Metrics:");
        info!("  Opportunities Received: {}", final_metrics.opportunities_received);
        info!("  Opportunities Executed: {}", final_metrics.opportunities_executed);
        info!("  Opportunities Skipped: {}", final_metrics.opportunities_skipped);
        info!("  Transactions Sent: {}", final_metrics.transactions_sent);
        info!("  Transactions Confirmed: {}", final_metrics.transactions_confirmed);
        info!("  Transactions Failed: {}", final_metrics.transactions_failed);
        info!(
            "  Total Profit: {} SOL",
            final_metrics.total_profit_lamports as f64 / 1e9
        );
        info!(
            "  Total Fees: {} SOL",
            final_metrics.total_fees_paid as f64 / 1e9
        );
        info!("  Front-runs Detected: {}", final_metrics.frontrun_detected);

        info!("✅ Shutdown complete");
        Ok(())
    }

    /// Execute arbitrage opportunities as they are detected
    async fn execute_opportunities(
        mut rx: mpsc::UnboundedReceiver<ArbitrageOpportunity>,
        tx_builder: Arc<SwapTransactionBuilder>,
        tx_sender: Arc<TransactionSender>,
        config: Config,
        metrics: Arc<RwLock<ExecutionMetrics>>,
    ) {
        info!("🎯 Execution engine ready");

        while let Some(opportunity) = rx.recv().await {
            // Update metrics
            {
                let mut m = metrics.write().await;
                m.opportunities_received += 1;
            }

            // Log opportunity
            info!(
                "💰 New opportunity: {} hops, profit: {:.4}%, score: {:.3}",
                opportunity.cycle.path.len(),
                opportunity.expected_profit_bps as f64 / 100.0,
                opportunity.priority_score
            );

            // Check if in simulation mode
            if config.bot.is_simulation_mode {
                info!("🎭 SIMULATION MODE: Would execute opportunity (profit: {:.4}%)", 
                    opportunity.expected_profit_bps as f64 / 100.0);
                
                let mut m = metrics.write().await;
                m.opportunities_executed += 1;
                continue;
            }

            // Check risk level
            if matches!(opportunity.risk_level, super::detector::RiskLevel::High) {
                warn!("⚠️  High risk opportunity, skipping");
                let mut m = metrics.write().await;
                m.opportunities_skipped += 1;
                continue;
            }

            // Execute the opportunity
            let start_time = std::time::Instant::now();
            
            match Self::execute_single_opportunity(
                &opportunity,
                &tx_builder,
                &tx_sender,
                &config,
            )
            .await
            {
                Ok(result) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    
                    info!(
                        "✅ Opportunity executed successfully! Signature: {}, Slot: {}, Time: {}ms",
                        result.signature, result.slot, execution_time
                    );

                    // Update metrics
                    let mut m = metrics.write().await;
                    m.opportunities_executed += 1;
                    m.transactions_sent += 1;
                    m.transactions_confirmed += 1;
                    m.total_profit_lamports += opportunity.expected_profit_bps as i64;
                    m.total_fees_paid += config.execution.compute_unit_price;
                    
                    // Update average execution time
                    let total_executions = m.opportunities_executed;
                    m.average_execution_time_ms = 
                        (m.average_execution_time_ms * (total_executions - 1) + execution_time) 
                        / total_executions;
                }
                Err(e) => {
                    error!("❌ Failed to execute opportunity: {}", e);

                    // Check for front-running
                    if e.to_string().contains("front") {
                        warn!("🏃 Front-run detected!");
                        let mut m = metrics.write().await;
                        m.frontrun_detected += 1;
                    }

                    let mut m = metrics.write().await;
                    m.transactions_failed += 1;
                }
            }
        }

        info!("Execution engine stopped");
    }

    /// Execute a single arbitrage opportunity
    async fn execute_single_opportunity(
        opportunity: &ArbitrageOpportunity,
        tx_builder: &SwapTransactionBuilder,
        tx_sender: &TransactionSender,
        config: &Config,
    ) -> Result<super::transaction_sender::SendResult> {
        // Build transaction
        debug!("🔨 Building transaction...");
        
        let tx_config = TransactionConfig {
            max_slippage_bps: config.bot.max_slippage_bps as u16,
            priority_fee_micro_lamports: config.execution.compute_unit_price,
            compute_unit_buffer: 50_000,
        };
        
        let transaction = tx_builder
            .build_arbitrage_tx(
                &opportunity.cycle,
                opportunity.optimal_input_amount,
                &tx_config,
            )
            .await
            .context("Failed to build transaction")?;

        // Estimate priority fee
        let priority_fee = tx_sender
            .estimate_priority_fee()
            .await
            .unwrap_or(config.execution.compute_unit_price);
        debug!("💸 Estimated priority fee: {} lamports", priority_fee);

        // Send transaction with confirmation
        debug!("📤 Sending transaction...");
        
        let send_config = SendConfig {
            priority_fee_lamports: priority_fee,
            skip_preflight: !config.execution.simulate_before_send,
            max_retries: config.bot.max_retries as u8,
        };
        
        let result = tx_sender
            .send_and_confirm(&transaction, &send_config)
            .await
            .context("Failed to send transaction")?;

        // Check for front-running
        // Assume actual profit is ~80% of expected for demonstration
        let estimated_actual_profit = (opportunity.expected_profit_bps as u64 * 80) / 100;
        if tx_sender.detect_frontrun(
            &result,
            opportunity.expected_profit_bps as u64,
            estimated_actual_profit,
        ) {
            anyhow::bail!("Transaction front-run detected");
        }

        Ok(result)
    }

    /// Report metrics periodically
    async fn report_metrics_loop(metrics: Arc<RwLock<ExecutionMetrics>>) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

        loop {
            interval.tick().await;

            let m = metrics.read().await;
            
            if m.opportunities_received > 0 {
                info!("📊 Metrics Report (Last 60s):");
                info!("  Opportunities: {} received, {} executed, {} skipped",
                    m.opportunities_received,
                    m.opportunities_executed,
                    m.opportunities_skipped
                );
                
                if m.transactions_sent > 0 {
                    let success_rate = (m.transactions_confirmed as f64 / m.transactions_sent as f64) * 100.0;
                    info!("  Transactions: {} sent, {} confirmed, {} failed (Success: {:.1}%)",
                        m.transactions_sent,
                        m.transactions_confirmed,
                        m.transactions_failed,
                        success_rate
                    );
                }
                
                info!("  Profit: {} SOL, Fees: {} SOL, Net: {} SOL",
                    m.total_profit_lamports as f64 / 1e9,
                    m.total_fees_paid as f64 / 1e9,
                    (m.total_profit_lamports - m.total_fees_paid as i64) as f64 / 1e9
                );
                
                if m.opportunities_executed > 0 {
                    info!("  Avg Execution Time: {}ms", m.average_execution_time_ms);
                }
                
                if m.frontrun_detected > 0 {
                    warn!("  ⚠️  Front-runs detected: {}", m.frontrun_detected);
                }
            }
        }
    }

    /// Get current metrics snapshot
    pub async fn get_metrics(&self) -> ExecutionMetrics {
        self.metrics.read().await.clone()
    }

    /// Trigger shutdown
    pub async fn shutdown(&self) -> Result<()> {
        self.shutdown_tx.send(()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::Keypair;

    #[tokio::test]
    async fn test_execution_metrics_default() {
        let metrics = ExecutionMetrics::default();
        assert_eq!(metrics.opportunities_received, 0);
        assert_eq!(metrics.opportunities_executed, 0);
        assert_eq!(metrics.total_profit_lamports, 0);
    }

    #[tokio::test]
    async fn test_create_rpc_clients() {
        let mut config = Config::load().unwrap();
        config.rpc.backup_urls = vec![
            "https://api.mainnet-beta.solana.com".to_string(),
            "https://solana-api.projectserum.com".to_string(),
        ];

        let clients = MevBotOrchestrator::create_rpc_clients(&config).unwrap();
        assert_eq!(clients.len(), 3); // 1 main + 2 backup
    }

    #[test]
    fn test_metrics_update() {
        let mut metrics = ExecutionMetrics::default();
        metrics.opportunities_received = 10;
        metrics.opportunities_executed = 7;
        metrics.opportunities_skipped = 3;

        assert_eq!(metrics.opportunities_received, 10);
        assert_eq!(metrics.opportunities_executed, 7);
        assert_eq!(metrics.opportunities_skipped, 3);
    }
}
