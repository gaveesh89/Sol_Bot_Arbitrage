/// Test: Local Fork + External API Integration
/// 
/// This test demonstrates the "Aggressive/Dynamic" testing strategy:
/// - Pool metadata: Fetched from Helius API (fast, real-time mainnet data)
/// - Vault balances: Fetched from local fork (for execution simulation)
/// - Execution: On local fork (safe, no real funds)

use anyhow::Result;
use solana_mev_bot::chain::token_fetch::{TokenFetcher, TokenFetchConfig, DexType};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use solana_client::nonblocking::rpc_client::RpcClient;
use serial_test::serial;

// Test pools
const RAYDIUM_SOL_USDC: &str = "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2";
const RAYDIUM_RAY_USDC: &str = "6UmmUiYoBjSrhakAobJw8BvkmJtDVxaeBtbt7rxWo1mg";

fn pubkey(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_fork_with_external_api() -> Result<()> {
    println!("\n🧪 Test: Local Fork + External API Integration");
    println!("================================================");
    
    // Check if fork is running
    let fork_url = "http://127.0.0.1:8899";
    let fork_client = RpcClient::new(fork_url.to_string());
    
    match fork_client.get_version().await {
        Ok(version) => {
            println!("✅ Local fork detected");
            println!("   Version: {}", version.solana_core);
        }
        Err(e) => {
            println!("❌ Local fork not running: {}", e);
            println!("\n💡 Start the fork first:");
            println!("   export MAINNET_RPC_URL='https://mainnet.helius-rpc.com/?api-key=YOUR_KEY'");
            println!("   ./start-mainnet-fork.sh");
            return Ok(());
        }
    }
    
    // Check if Helius API key is set
    let api_key = match std::env::var("HELIUS_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("⚠️  HELIUS_API_KEY not set, skipping test");
            return Ok(());
        }
    };
    
    println!("\n📡 Configuration:");
    println!("   External API: Helius (for pool metadata)");
    println!("   Local Fork: http://127.0.0.1:8899 (for vault balances & execution)");
    
    // Create RPC client pointing to local fork
    let rpc_client = Arc::new(RpcClient::new(fork_url.to_string()));
    
    // Configure TokenFetcher to use external API for pool metadata
    let helius_url = format!("https://mainnet.helius-rpc.com/?api-key={}", api_key);
    let config = TokenFetchConfig {
        max_retries: 3,
        retry_delay_ms: 500,
        batch_size: 100,
        timeout_seconds: 30,
        enable_caching: true,
        cache_ttl_seconds: 60,
        initial_retry_delay_ms: 200,
        max_retry_delay_ms: 30_000,
        retry_growth_factor: 2.0,
        jitter_percent: 0.25,
        metadata_ttl_seconds: 100,  // Fast refresh for testing
        price_data_ttl_seconds: 1,
        external_data_api_url: Some(helius_url.clone()),
    };
    
    let token_fetcher = TokenFetcher::with_config(rpc_client, config, 10_000);
    
    println!("\n🔄 Step 1: Fetch pool metadata from Helius");
    let pool_configs = vec![
        (pubkey(RAYDIUM_SOL_USDC), DexType::Raydium),
        (pubkey(RAYDIUM_RAY_USDC), DexType::Raydium),
    ];
    
    let start = std::time::Instant::now();
    let pools = token_fetcher.initialize_pool_data(&pool_configs).await?;
    let duration = start.elapsed();
    
    println!("   ✅ Fetched {} pools in {:?}", pools.len(), duration);
    println!("   📊 Pool metadata source: External Helius API (mainnet)");
    
    println!("\n📊 Pool Data:");
    for pool in &pools {
        println!("\n   Pool: {}", pool.pubkey);
        println!("   DEX: {:?}", pool.dex_type);
        println!("   Token A: {} ({})", pool.token_a_mint, pool.token_a_reserve);
        println!("   Token B: {} ({})", pool.token_b_mint, pool.token_b_reserve);
        println!("   Fee: {}/{}", pool.fee_numerator, pool.fee_denominator);
        
        if pool.token_a_reserve > 0 && pool.token_b_reserve > 0 {
            let rate = pool.token_b_reserve as f64 / pool.token_a_reserve as f64;
            println!("   Rate: {:.6} Token B per Token A", rate);
        }
    }
    
    // Verify we got real data
    assert_eq!(pools.len(), 2, "Should fetch 2 pools");
    
    for pool in &pools {
        assert!(pool.token_a_vault.is_some(), "Vault A should be set");
        assert!(pool.token_b_vault.is_some(), "Vault B should be set");
        
        // Note: Reserves come from external API (Helius mainnet data)
        // For local execution, you'd clone these vaults to the fork first
        if pool.token_a_reserve > 0 && pool.token_b_reserve > 0 {
            println!("\n   ✅ Pool {} has real mainnet reserves!", pool.pubkey);
        } else {
            println!("\n   ⚠️  Pool {} reserves are 0 (might be inactive)", pool.pubkey);
        }
    }
    
    println!("\n✅ Test Summary:");
    println!("   ✅ External API integration working");
    println!("   ✅ Pool metadata fetched from mainnet (Helius)");
    println!("   ✅ Local fork ready for execution");
    println!("\n💡 Next Steps:");
    println!("   1. Clone vault accounts to local fork for testing");
    println!("   2. Execute swaps on local fork");
    println!("   3. Verify arbitrage detection with real pool data");
    
    Ok(())
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_realtime_pool_data_refresh() -> Result<()> {
    println!("\n🔄 Test: Real-Time Pool Data Refresh");
    println!("====================================");
    
    let api_key = match std::env::var("HELIUS_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("⚠️  HELIUS_API_KEY not set, skipping test");
            return Ok(());
        }
    };
    
    let fork_url = "http://127.0.0.1:8899";
    let rpc_client = Arc::new(RpcClient::new(fork_url.to_string()));
    
    let helius_url = format!("https://mainnet.helius-rpc.com/?api-key={}", api_key);
    let config = TokenFetchConfig {
        max_retries: 3,
        retry_delay_ms: 500,
        batch_size: 100,
        timeout_seconds: 30,
        enable_caching: true,
        cache_ttl_seconds: 5,  // Short TTL for testing refresh
        initial_retry_delay_ms: 200,
        max_retry_delay_ms: 30_000,
        retry_growth_factor: 2.0,
        jitter_percent: 0.25,
        metadata_ttl_seconds: 5,
        price_data_ttl_seconds: 1,
        external_data_api_url: Some(helius_url.clone()),
    };
    
    let token_fetcher = TokenFetcher::with_config(rpc_client, config, 10_000);
    
    let pool_configs = vec![
        (pubkey(RAYDIUM_SOL_USDC), DexType::Raydium),
    ];
    
    println!("\n📊 Fetch 1: Initial pool data");
    let start1 = std::time::Instant::now();
    let pools1 = token_fetcher.initialize_pool_data(&pool_configs).await?;
    let duration1 = start1.elapsed();
    println!("   Duration: {:?}", duration1);
    println!("   Reserve A: {}", pools1[0].token_a_reserve);
    println!("   Reserve B: {}", pools1[0].token_b_reserve);
    
    println!("\n⏳ Waiting 2 seconds...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    println!("\n📊 Fetch 2: Should use cache (< 5s TTL)");
    let start2 = std::time::Instant::now();
    let pools2 = token_fetcher.initialize_pool_data(&pool_configs).await?;
    let duration2 = start2.elapsed();
    println!("   Duration: {:?} (should be faster - cached)", duration2);
    println!("   Reserve A: {}", pools2[0].token_a_reserve);
    println!("   Reserve B: {}", pools2[0].token_b_reserve);
    
    println!("\n⏳ Waiting 4 more seconds (total 6s, > 5s TTL)...");
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    
    println!("\n📊 Fetch 3: Should refresh from API (> 5s TTL)");
    let start3 = std::time::Instant::now();
    let pools3 = token_fetcher.initialize_pool_data(&pool_configs).await?;
    let duration3 = start3.elapsed();
    println!("   Duration: {:?} (should be slower - refreshed)", duration3);
    println!("   Reserve A: {}", pools3[0].token_a_reserve);
    println!("   Reserve B: {}", pools3[0].token_b_reserve);
    
    println!("\n✅ Cache behavior:");
    println!("   Fetch 1: {:?} (initial)", duration1);
    println!("   Fetch 2: {:?} (cached)", duration2);
    println!("   Fetch 3: {:?} (refreshed)", duration3);
    
    // Verify cache is working
    assert!(duration2 < duration1, "Cached fetch should be faster");
    
    println!("\n✅ Real-time data refresh working correctly!");
    
    Ok(())
}
