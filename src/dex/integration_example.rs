// Integration Example: Pool Fetcher + Arbitrage Graph
//
// This example demonstrates how to use PoolDataFetcher to populate
// an ArbitrageGraph for triangular arbitrage detection.

use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::str::FromStr;

use crate::dex::pool_fetcher::{PoolDataFetcher, PoolData};
use crate::dex::triangular_arb::{
    create_shared_graph, ExchangeEdge, BellmanFordDetector
};

/// Example: Fetch pools and populate arbitrage graph
pub async fn fetch_and_populate_graph_example() -> Result<()> {
    // Step 1: Initialize RPC clients with failover
    let rpc_urls = vec![
        "http://localhost:8899",  // Local fork
        "https://api.mainnet-beta.solana.com",
        "https://api.devnet.solana.com",
    ];
    
    let rpc_clients: Vec<Arc<RpcClient>> = rpc_urls
        .iter()
        .map(|url| Arc::new(RpcClient::new(url.to_string())))
        .collect();
    
    // Step 2: Create pool data fetcher with 60-second cache TTL
    let fetcher = PoolDataFetcher::new(rpc_clients, 60_000);
    
    // Step 3: Define pool addresses to monitor
    let pool_addresses = vec![
        // Example: SOL/USDC pools on different DEXs
        Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?, // Raydium SOL/USDC
        Pubkey::from_str("7qbRF6YsyGuLUVs6Y1q64bdVrfe4ZcUUz1JRdoVNUJnm")?, // Orca SOL/USDC
        // Add more pools...
    ];
    
    // Step 4: Fetch pool data in batch
    println!("Fetching {} pools...", pool_addresses.len());
    let pools = fetcher.fetch_pools_batch(&pool_addresses).await?;
    println!("Successfully fetched {} pools", pools.len());
    
    // Step 5: Create arbitrage graph
    let graph = create_shared_graph();
    
    // Step 6: Populate graph with pool data
    {
        let mut g = graph.write().unwrap();
        for pool in &pools {
            // Add edge: token A -> token B
            let edge_a_to_b = ExchangeEdge::new(
                pool.token_a,
                pool.token_b,
                pool.dex_type.clone(),
                pool.pool_address,
                pool.calculate_rate_a_to_b(),
                pool.fee_bps,
                vec![], // Liquidity depth can be added later
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            );
            g.add_edge(edge_a_to_b);
            
            // Add edge: token B -> token A
            let edge_b_to_a = ExchangeEdge::new(
                pool.token_b,
                pool.token_a,
                pool.dex_type.clone(),
                pool.pool_address,
                pool.calculate_rate_b_to_a(),
                pool.fee_bps,
                vec![],
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            );
            g.add_edge(edge_b_to_a);
        }
        
        println!("Graph populated: {} tokens, {} edges", 
            g.token_count(), g.edge_count());
    }
    
    // Step 7: Create Bellman-Ford detector
    let detector = BellmanFordDetector::new(Arc::clone(&graph), 50) // 50 bps = 0.5% min profit
        .with_max_path_length(3);
    
    // Step 8: Detect arbitrage opportunities
    let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
    println!("Detecting arbitrage from USDC...");
    let cycles = detector.detect_arbitrage(usdc_mint).await?;
    
    println!("Found {} arbitrage opportunities", cycles.len());
    for (i, cycle) in cycles.iter().enumerate() {
        println!("  Opportunity #{}: {} bps profit, {} hops", 
            i + 1, cycle.gross_profit_bps, cycle.path.len());
    }
    
    Ok(())
}

/// Continuous monitoring loop with periodic refresh
pub async fn continuous_monitoring_example() -> Result<()> {
    use tokio::time::{interval, Duration};
    
    // Initialize fetcher and graph
    let rpc_clients = vec![
        Arc::new(RpcClient::new("http://localhost:8899".to_string())),
    ];
    let fetcher = PoolDataFetcher::new(rpc_clients, 5_000); // 5 second cache
    let graph = create_shared_graph();
    let detector = BellmanFordDetector::new(Arc::clone(&graph), 50);
    
    // Pool addresses to monitor
    let pool_addresses = vec![
        // Your pools here
    ];
    
    // Monitoring loop
    let mut tick_interval = interval(Duration::from_secs(1));
    let mut refresh_interval = interval(Duration::from_secs(5));
    
    loop {
        tokio::select! {
            // Every 5 seconds: refresh pool data
            _ = refresh_interval.tick() => {
                match fetcher.fetch_pools_batch(&pool_addresses).await {
                    Ok(pools) => {
                        // Update graph with fresh data
                        let mut g = graph.write().unwrap();
                        for pool in &pools {
                            // Update exchange rates
                            let timestamp = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs() as i64;
                            
                            let _ = g.update_edge_rate(
                                pool.token_a,
                                pool.token_b,
                                pool.dex_type.clone(),
                                pool.calculate_rate_a_to_b(),
                                timestamp,
                            );
                            
                            let _ = g.update_edge_rate(
                                pool.token_b,
                                pool.token_a,
                                pool.dex_type.clone(),
                                pool.calculate_rate_b_to_a(),
                                timestamp,
                            );
                        }
                        
                        println!("Updated {} pools", pools.len());
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch pools: {}", e);
                    }
                }
                
                // Clear expired cache entries
                fetcher.clear_expired_cache().await;
            }
            
            // Every second: detect arbitrage
            _ = tick_interval.tick() => {
                let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
                
                match detector.detect_arbitrage(usdc).await {
                    Ok(cycles) => {
                        for cycle in cycles {
                            if cycle.net_profit_after_fees > 0.005 && 
                               cycle.fits_in_transaction() {
                                println!("🚨 ARBITRAGE: {} bps profit", cycle.gross_profit_bps);
                                // Execute trade here
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Detection failed: {}", e);
                    }
                }
            }
        }
    }
}

/// Advanced: Parallel detection across multiple tokens
pub async fn parallel_monitoring_example() -> Result<()> {
    let rpc_clients = vec![
        Arc::new(RpcClient::new("http://localhost:8899".to_string())),
    ];
    let _fetcher = PoolDataFetcher::new(rpc_clients, 10_000);
    let graph = create_shared_graph();
    let detector = BellmanFordDetector::new(Arc::clone(&graph), 30);
    
    // Major tokens to monitor
    let start_tokens = vec![
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?, // USDC
        Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB")?, // USDT
        Pubkey::from_str("So11111111111111111111111111111111111111112")?, // SOL
    ];
    
    // Detect from all tokens in parallel
    println!("Detecting arbitrage from {} tokens...", start_tokens.len());
    let all_cycles = detector.detect_arbitrage_parallel(start_tokens).await?;
    
    println!("Found {} total opportunities", all_cycles.len());
    
    // Print top 10
    for (i, cycle) in all_cycles.iter().take(10).enumerate() {
        println!("  #{}: {} bps, {} hops, {}ms execution", 
            i + 1, 
            cycle.gross_profit_bps, 
            cycle.path.len(),
            cycle.execution_time_estimate_ms
        );
    }
    
    Ok(())
}

/// Calculate optimal trade size with liquidity constraints
pub fn calculate_optimal_trade_size_example(pool: &PoolData) -> u64 {
    // Example: Calculate max trade size for 1% slippage
    let max_slippage = 0.01; // 1%
    
    // For constant product AMM: slippage = amount_in / (reserve_in + amount_in)
    // Solving for amount_in: amount_in = (slippage * reserve_in) / (1 - slippage)
    
    let max_amount_a = ((max_slippage * pool.reserve_a as f64) / (1.0 - max_slippage)) as u64;
    let max_amount_b = ((max_slippage * pool.reserve_b as f64) / (1.0 - max_slippage)) as u64;
    
    // Return the more conservative amount
    max_amount_a.min(max_amount_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_optimal_trade_size() {
        let pool = PoolData {
            pool_address: Pubkey::new_unique(),
            token_a: Pubkey::new_unique(),
            token_b: Pubkey::new_unique(),
            reserve_a: 1_000_000_000, // 1B
            reserve_b: 2_000_000_000, // 2B
            fee_bps: 30,
            dex_type: crate::dex::triangular_arb::DexType::Raydium,
            program_id: Pubkey::new_unique(),
        };
        
        let optimal = calculate_optimal_trade_size_example(&pool);
        
        // Should be around 1% of smaller reserve
        assert!(optimal > 9_000_000); // At least 0.9%
        assert!(optimal < 11_000_000); // At most 1.1%
    }
}
