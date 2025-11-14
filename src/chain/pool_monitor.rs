// WebSocket-based Real-Time Pool Monitoring
//
// This module implements continuous monitoring of pool account changes using
// Solana's WebSocket subscriptions (accountSubscribe). It automatically updates
// the arbitrage graph and triggers detection when profitable opportunities appear.
//
// Features:
// - Real-time account change subscriptions
// - Automatic graph updates on pool state changes
// - Rate-limited arbitrage detection
// - Automatic reconnection on disconnect
// - Concurrent subscription management

use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::rpc_config::RpcAccountInfoConfig;
use solana_sdk::{
    account::Account,
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
};
use tokio::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Result, anyhow};
use tracing::{debug, warn, info, error};

use crate::dex::triangular_arb::{SharedArbitrageGraph, BellmanFordDetector};
use crate::dex::pool_fetcher::{PoolDataFetcher, PoolData};

/// Configuration for pool monitoring
#[derive(Clone, Debug)]
pub struct MonitorConfig {
    pub detection_rate_limit_ms: u64,  // Min time between detections
    pub max_reconnect_attempts: usize,
    pub reconnect_delay_ms: u64,
    pub subscription_batch_size: usize,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            detection_rate_limit_ms: 1000,  // 1 second
            max_reconnect_attempts: 10,
            reconnect_delay_ms: 2000,        // 2 seconds
            subscription_batch_size: 50,     // Subscribe 50 at a time
        }
    }
}

/// Pool update event from WebSocket
#[derive(Clone, Debug)]
pub struct PoolUpdate {
    pub pool_address: Pubkey,
    pub new_data: PoolData,
    pub slot: u64,
    pub timestamp: i64,
}

/// Pool monitor with WebSocket subscriptions
pub struct PoolMonitor {
    pubsub_url: String,
    graph: SharedArbitrageGraph,
    pool_fetcher: Arc<PoolDataFetcher>,
    monitored_pools: Vec<Pubkey>,
    config: MonitorConfig,
    detector: Arc<BellmanFordDetector>,
}

impl PoolMonitor {
    /// Create a new pool monitor
    pub fn new(
        pubsub_url: String,
        graph: SharedArbitrageGraph,
        pool_fetcher: Arc<PoolDataFetcher>,
        monitored_pools: Vec<Pubkey>,
        detector: Arc<BellmanFordDetector>,
    ) -> Self {
        info!("Initializing PoolMonitor for {} pools", monitored_pools.len());
        
        Self {
            pubsub_url,
            graph,
            pool_fetcher,
            monitored_pools,
            config: MonitorConfig::default(),
            detector,
        }
    }
    
    /// Create with custom configuration
    pub fn with_config(mut self, config: MonitorConfig) -> Self {
        self.config = config;
        self
    }
    
    /// Start monitoring all pools with WebSocket subscriptions
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting WebSocket pool monitoring...");
        info!("  Pools: {}", self.monitored_pools.len());
        info!("  Detection rate limit: {}ms", self.config.detection_rate_limit_ms);
        info!("  WebSocket URL: {}", self.pubsub_url);
        
        // Create channel for pool updates
        let (tx, rx) = mpsc::unbounded_channel();
        
        // Start update processor task
        let processor_handle = {
            let monitor = self.clone_for_task();
            tokio::spawn(async move {
                monitor.process_updates(rx).await;
            })
        };
        
        // Subscribe to all pools with reconnection logic
        let subscription_handle = {
            let monitor = self.clone_for_task();
            let tx = tx.clone();
            tokio::spawn(async move {
                monitor.manage_subscriptions(tx).await;
            })
        };
        
        // Wait for both tasks (or until one fails)
        tokio::select! {
            _ = processor_handle => {
                error!("Update processor task exited");
            }
            _ = subscription_handle => {
                error!("Subscription manager task exited");
            }
        }
        
        Ok(())
    }
    
    /// Manage WebSocket subscriptions with automatic reconnection
    async fn manage_subscriptions(&self, tx: mpsc::UnboundedSender<PoolUpdate>) {
        let mut reconnect_count = 0;
        
        loop {
            match self.subscribe_all_pools(tx.clone()).await {
                Ok(_) => {
                    info!("All subscriptions completed successfully");
                    reconnect_count = 0;
                }
                Err(e) => {
                    reconnect_count += 1;
                    error!("Subscription error (attempt {}/{}): {}", 
                        reconnect_count, self.config.max_reconnect_attempts, e);
                    
                    if reconnect_count >= self.config.max_reconnect_attempts {
                        error!("Max reconnection attempts reached, giving up");
                        break;
                    }
                    
                    // Wait before reconnecting
                    warn!("Reconnecting in {}ms...", self.config.reconnect_delay_ms);
                    tokio::time::sleep(Duration::from_millis(self.config.reconnect_delay_ms)).await;
                }
            }
        }
    }
    
    /// Subscribe to all monitored pools
    async fn subscribe_all_pools(&self, tx: mpsc::UnboundedSender<PoolUpdate>) -> Result<()> {
        info!("Connecting to WebSocket: {}", self.pubsub_url);
        
        let pubsub_client = Arc::new(PubsubClient::new(&self.pubsub_url).await
            .map_err(|e| anyhow!("Failed to connect to WebSocket: {}", e))?);
        
        info!("WebSocket connected, subscribing to {} pools", self.monitored_pools.len());
        
        // Batch subscriptions to avoid overwhelming the connection
        for (i, chunk) in self.monitored_pools.chunks(self.config.subscription_batch_size).enumerate() {
            debug!("Subscribing batch {}: {} pools", i + 1, chunk.len());
            
            for pool_address in chunk {
                let tx_clone = tx.clone();
                let pool = *pool_address;
                let pool_fetcher = Arc::clone(&self.pool_fetcher);
                let pubsub_client = Arc::clone(&pubsub_client);
                
                // Spawn subscription task for each pool
                tokio::spawn(async move {
                    if let Err(e) = Self::subscribe_single_pool(
                        pubsub_client,
                        pool,
                        tx_clone,
                        pool_fetcher,
                    ).await {
                        warn!("Subscription failed for pool {}: {}", pool, e);
                    }
                });
            }
            
            // Small delay between batches
            if i < self.monitored_pools.chunks(self.config.subscription_batch_size).len() - 1 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        
        info!("All pool subscriptions initiated");
        
        // Keep connection alive
        tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
        
        Ok(())
    }
    
    /// Subscribe to a single pool account
    async fn subscribe_single_pool(
        pubsub_client: Arc<PubsubClient>,
        pool_address: Pubkey,
        tx: mpsc::UnboundedSender<PoolUpdate>,
        pool_fetcher: Arc<PoolDataFetcher>,
    ) -> Result<()> {
        let config = RpcAccountInfoConfig {
            encoding: None,
            commitment: Some(CommitmentConfig::confirmed()),
            data_slice: None,
            min_context_slot: None,
        };
        
        debug!("Subscribing to pool: {}", pool_address);
        
        let (mut stream, _unsubscribe) = pubsub_client
            .account_subscribe(&pool_address, Some(config))
            .await
            .map_err(|e| anyhow!("Failed to subscribe to {}: {}", pool_address, e))?;
        
        info!("Subscribed to pool: {}", pool_address);
        
        // Process updates from this subscription
        use tokio_stream::StreamExt as _;
        loop {
            match tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
                Ok(Some(response)) => {
                    let slot = response.context.slot;
                    let ui_account = response.value;
                    
                    debug!("Received update for pool {} at slot {}", pool_address, slot);
                    
                    // Decode UiAccount to Account
                    if let Some(account) = ui_account.decode() {
                        // Parse pool data
                        match Self::parse_pool_update(&pool_address, &account, slot, &pool_fetcher).await {
                            Ok(update) => {
                                if let Err(e) = tx.send(update) {
                                    error!("Failed to send pool update: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse pool update for {}: {}", pool_address, e);
                            }
                        }
                    } else {
                        warn!("Failed to decode account for pool: {}", pool_address);
                    }
                }
                Ok(None) => {
                    warn!("Subscription stream ended for pool: {}", pool_address);
                    break;
                }
                Err(_) => {
                    debug!("No update received in 30s for pool: {}", pool_address);
                    continue;
                }
            }
        }
        
        warn!("Subscription stream ended for pool: {}", pool_address);
        Ok(())
    }
    
    /// Parse pool account update
    async fn parse_pool_update(
        pool_address: &Pubkey,
        account: &Account,
        slot: u64,
        _pool_fetcher: &PoolDataFetcher,
    ) -> Result<PoolUpdate> {
        // Parse pool data directly based on owner program
        let owner = account.owner;
        
        // Simplified parsing - in production, use proper DEX-specific parsers
        let pool_data = PoolData {
            pool_address: *pool_address,
            token_a: Pubkey::default(),
            token_b: Pubkey::default(),
            reserve_a: 0,
            reserve_b: 0,
            fee_bps: 30,
            dex_type: crate::dex::triangular_arb::DexType::Raydium,
            program_id: owner,
        };
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        Ok(PoolUpdate {
            pool_address: *pool_address,
            new_data: pool_data,
            slot,
            timestamp,
        })
    }
    
    /// Process pool updates and trigger arbitrage detection
    async fn process_updates(&self, mut rx: mpsc::UnboundedReceiver<PoolUpdate>) {
        info!("Starting pool update processor");
        
        let mut last_detection = Instant::now();
        let mut updates_since_detection = 0;
        let rate_limit_duration = Duration::from_millis(self.config.detection_rate_limit_ms);
        
        while let Some(update) = rx.recv().await {
            updates_since_detection += 1;
            
            // Update graph with new pool data
            if let Err(e) = self.update_graph(&update).await {
                warn!("Failed to update graph for pool {}: {}", update.pool_address, e);
                continue;
            }
            
            debug!("Updated graph with pool {} data from slot {}", 
                update.pool_address, update.slot);
            
            // Check if enough time has passed since last detection
            let now = Instant::now();
            if now.duration_since(last_detection) >= rate_limit_duration {
                info!("Triggering arbitrage detection ({} updates accumulated)", 
                    updates_since_detection);
                
                // Trigger detection in background
                let detector = Arc::clone(&self.detector);
                let usdc_mint = solana_sdk::pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
                
                tokio::spawn(async move {
                    match detector.detect_arbitrage(usdc_mint).await {
                        Ok(cycles) => {
                            if !cycles.is_empty() {
                                info!("🎯 Found {} arbitrage opportunities!", cycles.len());
                                for (i, cycle) in cycles.iter().take(5).enumerate() {
                                    info!("  #{}: {} bps profit, {} hops", 
                                        i + 1, cycle.gross_profit_bps, cycle.path.len());
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Arbitrage detection failed: {}", e);
                        }
                    }
                });
                
                last_detection = now;
                updates_since_detection = 0;
            }
        }
        
        warn!("Pool update processor exiting");
    }
    
    /// Update arbitrage graph with new pool data
    async fn update_graph(&self, update: &PoolUpdate) -> Result<()> {
        let pool = &update.new_data;
        
        let mut graph = self.graph.write()
            .map_err(|e| anyhow!("Failed to acquire graph lock: {}", e))?;
        
        // Update edge: token A -> token B
        graph.update_edge_rate(
            pool.token_a,
            pool.token_b,
            pool.dex_type.clone(),
            pool.calculate_rate_a_to_b(),
            update.timestamp,
        )?;
        
        // Update edge: token B -> token A
        graph.update_edge_rate(
            pool.token_b,
            pool.token_a,
            pool.dex_type.clone(),
            pool.calculate_rate_b_to_a(),
            update.timestamp,
        )?;
        
        Ok(())
    }
    
    /// Clone for spawning async tasks
    fn clone_for_task(&self) -> Self {
        Self {
            pubsub_url: self.pubsub_url.clone(),
            graph: Arc::clone(&self.graph),
            pool_fetcher: Arc::clone(&self.pool_fetcher),
            monitored_pools: self.monitored_pools.clone(),
            config: self.config.clone(),
            detector: Arc::clone(&self.detector),
        }
    }
    
    /// Get monitoring statistics
    pub async fn get_stats(&self) -> MonitorStats {
        let graph = self.graph.read().unwrap();
        
        MonitorStats {
            monitored_pools: self.monitored_pools.len(),
            graph_tokens: graph.token_count(),
            graph_edges: graph.edge_count(),
            rate_limit_ms: self.config.detection_rate_limit_ms,
        }
    }
}

/// Monitoring statistics
#[derive(Clone, Debug)]
pub struct MonitorStats {
    pub monitored_pools: usize,
    pub graph_tokens: usize,
    pub graph_edges: usize,
    pub rate_limit_ms: u64,
}

/// Batch pool monitor for efficient subscription management
pub struct BatchPoolMonitor {
    monitors: Vec<PoolMonitor>,
}

impl BatchPoolMonitor {
    /// Create monitors for multiple WebSocket URLs (sharding)
    pub fn new(
        pubsub_urls: Vec<String>,
        graph: SharedArbitrageGraph,
        pool_fetcher: Arc<PoolDataFetcher>,
        all_pools: Vec<Pubkey>,
        detector: Arc<BellmanFordDetector>,
    ) -> Self {
        let pools_per_monitor = (all_pools.len() + pubsub_urls.len() - 1) / pubsub_urls.len();
        
        let monitors: Vec<PoolMonitor> = pubsub_urls
            .into_iter()
            .enumerate()
            .map(|(i, url)| {
                let start = i * pools_per_monitor;
                let end = ((i + 1) * pools_per_monitor).min(all_pools.len());
                let pools = all_pools[start..end].to_vec();
                
                PoolMonitor::new(
                    url,
                    Arc::clone(&graph),
                    Arc::clone(&pool_fetcher),
                    pools,
                    Arc::clone(&detector),
                )
            })
            .collect();
        
        info!("Created {} batch monitors", monitors.len());
        
        Self { monitors }
    }
    
    /// Start all monitors concurrently
    pub async fn start_all(&self) -> Result<()> {
        let mut handles = Vec::new();
        
        for monitor in &self.monitors {
            let monitor = monitor.clone_for_task();
            let handle = tokio::spawn(async move {
                if let Err(e) = monitor.start_monitoring().await {
                    error!("Monitor failed: {}", e);
                }
            });
            handles.push(handle);
        }
        
        // Wait for all monitors
        for handle in handles {
            handle.await.map_err(|e| anyhow!("Task failed: {}", e))?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dex::triangular_arb::create_shared_graph;
    
    fn create_test_pubkey(seed: u8) -> Pubkey {
        Pubkey::new_from_array([seed; 32])
    }
    
    #[test]
    fn test_monitor_config_default() {
        let config = MonitorConfig::default();
        assert_eq!(config.detection_rate_limit_ms, 1000);
        assert_eq!(config.max_reconnect_attempts, 10);
        assert_eq!(config.reconnect_delay_ms, 2000);
        assert_eq!(config.subscription_batch_size, 50);
    }
    
    #[test]
    fn test_pool_update_creation() {
        use crate::dex::pool_fetcher::PoolData;
        use crate::dex::triangular_arb::DexType;
        
        let pool_data = PoolData {
            pool_address: create_test_pubkey(1),
            token_a: create_test_pubkey(2),
            token_b: create_test_pubkey(3),
            reserve_a: 1000000,
            reserve_b: 2000000,
            fee_bps: 25,
            dex_type: DexType::Raydium,
            program_id: create_test_pubkey(100),
        };
        
        let update = PoolUpdate {
            pool_address: pool_data.pool_address,
            new_data: pool_data.clone(),
            slot: 12345,
            timestamp: 1699999999,
        };
        
        assert_eq!(update.slot, 12345);
        assert_eq!(update.new_data.reserve_a, 1000000);
    }
    
    #[test]
    fn test_monitor_stats() {
        let stats = MonitorStats {
            monitored_pools: 100,
            graph_tokens: 50,
            graph_edges: 200,
            rate_limit_ms: 1000,
        };
        
        assert_eq!(stats.monitored_pools, 100);
        assert_eq!(stats.graph_tokens, 50);
        assert_eq!(stats.graph_edges, 200);
    }
    
    #[tokio::test]
    async fn test_batch_monitor_creation() {
        use solana_client::nonblocking::rpc_client::RpcClient;
        
        let graph = create_shared_graph();
        let rpc_clients = vec![Arc::new(RpcClient::new("http://localhost:8899".to_string()))];
        let pool_fetcher = Arc::new(PoolDataFetcher::new(rpc_clients, 60000));
        let detector = Arc::new(BellmanFordDetector::new(Arc::clone(&graph), 50));
        
        let pools = vec![create_test_pubkey(1), create_test_pubkey(2), create_test_pubkey(3)];
        let urls = vec!["ws://localhost:8900".to_string()];
        
        let batch_monitor = BatchPoolMonitor::new(
            urls,
            graph,
            pool_fetcher,
            pools,
            detector,
        );
        
        assert_eq!(batch_monitor.monitors.len(), 1);
    }
}
