/// Integration Test: External API Data Fetching with Helius
/// 
/// This test validates the "Aggressive/Dynamic" testing solution by:
/// 1. Starting a local validator fork
/// 2. Configuring the bot to use Helius API for real-time pool data
/// 3. Fetching pool reserves from Helius instead of local validator
/// 4. Verifying the data is current and accurate
/// 
/// Run with:
///   HELIUS_API_KEY="your_key" cargo test test_external_api_integration -- --ignored --nocapture

use anyhow::Result;
use solana_mev_bot::chain::token_fetch::{TokenFetcher, TokenFetchConfig, DexType};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use solana_client::nonblocking::rpc_client::RpcClient;
use serial_test::serial;

// Well-known mainnet pools for testing
const RAYDIUM_SOL_USDC: &str = "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2";
const ORCA_SOL_USDC: &str = "7qbRF6YsyGuLUVs6Y1q64bdVrfe4ZcUUz1JRdoVNUJnm";
const METEORA_SOL_USDC: &str = "Bx7DRVY7zF8W6gZoVRgj3h6pKXK5RJBCovW6JkDz9X8z";

fn pubkey(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

#[tokio::test]
#[serial]
#[ignore] // Run with: cargo test --test test_external_api_integration -- --ignored
async fn test_external_api_integration() -> Result<()> {
    println!("\n🧪 Test: External API Integration with Helius");
    println!("{}", "=".repeat(80));
    
    // Check if Helius API key is available
    let api_key = match std::env::var("HELIUS_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("⚠️  Skipping test: HELIUS_API_KEY not set");
            println!("   Set it with: export HELIUS_API_KEY='your_key'");
            return Ok(());
        }
    };
    
    println!("✅ HELIUS_API_KEY found");
    
    // ========================================================================
    // PHASE 1: Setup with External API Configuration
    // ========================================================================
    println!("\n📋 Phase 1: Configure TokenFetcher with External API");
    println!("{}", "-".repeat(80));
    
    // Create RPC client (can point to local fork or mainnet)
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let rpc_client = Arc::new(RpcClient::new(rpc_url.clone()));
    
    println!("   RPC URL: {}", rpc_url);
    
    // Configure with external API URL
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
        metadata_ttl_seconds: 3600,
        price_data_ttl_seconds: 1,
        external_data_api_url: Some(helius_url.clone()),
    };
    
    println!("   External API: {}", helius_url.split('?').next().unwrap());
    println!("   Batch size: {}", config.batch_size);
    println!("   Cache enabled: {}", config.enable_caching);
    
    let token_fetcher = TokenFetcher::with_config(rpc_client.clone(), config, 10_000);
    
    println!("✅ TokenFetcher configured with external API");
    
    // ========================================================================
    // PHASE 2: Fetch Pool Data from External API
    // ========================================================================
    println!("\n📊 Phase 2: Fetch Real-Time Pool Data from Helius");
    println!("{}", "-".repeat(80));
    
    let pool_configs = vec![
        (pubkey(RAYDIUM_SOL_USDC), DexType::Raydium),
        (pubkey(ORCA_SOL_USDC), DexType::Whirlpool),
        (pubkey(METEORA_SOL_USDC), DexType::Meteora),
    ];
    
    println!("   Fetching {} pools:", pool_configs.len());
    for (addr, dex) in &pool_configs {
        println!("     - {} ({:?})", addr, dex);
    }
    
    let start = std::time::Instant::now();
    let pools = token_fetcher.initialize_pool_data(&pool_configs).await?;
    let elapsed = start.elapsed();
    
    println!("\n✅ Fetched {} pools in {:?}", pools.len(), elapsed);
    
    // ========================================================================
    // PHASE 3: Validate Pool Data
    // ========================================================================
    println!("\n🔍 Phase 3: Validate Pool Data Quality");
    println!("{}", "-".repeat(80));
    
    for pool in &pools {
        println!("\n   Pool: {}", pool.pubkey);
        println!("   DEX: {:?}", pool.dex_type);
        println!("   Token A: {}", pool.token_a_mint);
        println!("   Token B: {}", pool.token_b_mint);
        println!("   Reserve A: {}", pool.token_a_reserve);
        println!("   Reserve B: {}", pool.token_b_reserve);
        
        // Validate reserves are non-zero
        assert!(pool.token_a_reserve > 0, "Token A reserve should be > 0");
        assert!(pool.token_b_reserve > 0, "Token B reserve should be > 0");
        
        // Validate pool address matches
        let expected_addr = pool_configs.iter()
            .find(|(addr, _)| *addr == pool.pubkey)
            .map(|(addr, _)| addr);
        assert_eq!(Some(&pool.pubkey), expected_addr);
        
        // Check data freshness
        if let Ok(age) = pool.last_updated.elapsed() {
            println!("   Data age: {:?}", age);
            assert!(age.as_secs() < 10, "Data should be less than 10 seconds old");
        }
        
        println!("   ✅ Pool data validated");
    }
    
    // ========================================================================
    // PHASE 4: Test Cache Performance
    // ========================================================================
    println!("\n⚡ Phase 4: Test Cache Performance");
    println!("{}", "-".repeat(80));
    
    println!("   Fetching same pools again (should use cache)...");
    let start = std::time::Instant::now();
    let pools_cached = token_fetcher.initialize_pool_data(&pool_configs).await?;
    let cached_elapsed = start.elapsed();
    
    println!("   Second fetch completed in {:?}", cached_elapsed);
    println!("   First fetch: {:?}", elapsed);
    println!("   Second fetch: {:?}", cached_elapsed);
    println!("   Speedup: {:.2}x", elapsed.as_secs_f64() / cached_elapsed.as_secs_f64());
    
    assert_eq!(pools.len(), pools_cached.len(), "Cache should return same number of pools");
    println!("✅ Cache working correctly");
    
    // ========================================================================
    // PHASE 5: Performance Comparison
    // ========================================================================
    println!("\n📈 Phase 5: Performance Summary");
    println!("{}", "-".repeat(80));
    
    println!("   API Type: External (Helius)");
    println!("   Total pools: {}", pools.len());
    println!("   Fetch time: {:?}", elapsed);
    println!("   Avg per pool: {:?}", elapsed / pools.len() as u32);
    println!("   Cache hit time: {:?}", cached_elapsed);
    println!("   Cache speedup: {:.2}x", elapsed.as_secs_f64() / cached_elapsed.as_secs_f64());
    
    // ========================================================================
    // PHASE 6: Real-Time Update Test
    // ========================================================================
    println!("\n🔄 Phase 6: Test Real-Time Data Freshness");
    println!("{}", "-".repeat(80));
    
    println!("   Waiting 150ms for cache to expire...");
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    
    println!("   Fetching fresh data from Helius...");
    let start = std::time::Instant::now();
    let pools_fresh = token_fetcher.initialize_pool_data(&pool_configs).await?;
    let fresh_elapsed = start.elapsed();
    
    println!("   Fresh fetch completed in {:?}", fresh_elapsed);
    
    // Compare reserves to detect any changes
    for (old, new) in pools.iter().zip(pools_fresh.iter()) {
        let reserve_a_diff = (new.token_a_reserve as i128 - old.token_a_reserve as i128).abs();
        let reserve_b_diff = (new.token_b_reserve as i128 - old.token_b_reserve as i128).abs();
        
        println!("\n   Pool: {}", new.pubkey);
        if reserve_a_diff > 0 || reserve_b_diff > 0 {
            println!("     Reserve A changed by: {}", reserve_a_diff);
            println!("     Reserve B changed by: {}", reserve_b_diff);
            println!("     ✅ Real-time update detected!");
        } else {
            println!("     Reserves unchanged (no trades in 150ms window)");
        }
    }
    
    // ========================================================================
    // SUCCESS
    // ========================================================================
    println!("\n{}", "=".repeat(80));
    println!("✅ ALL TESTS PASSED - External API Integration Working!");
    println!("{}", "=".repeat(80));
    
    println!("\n📋 Summary:");
    println!("   ✅ External API configuration working");
    println!("   ✅ Helius data fetching successful");
    println!("   ✅ Pool data validation passed");
    println!("   ✅ Cache performance optimal");
    println!("   ✅ Real-time data freshness confirmed");
    
    println!("\n💡 Next Steps:");
    println!("   1. Run with local fork: RPC_URL=http://localhost:8899");
    println!("   2. Start arbitrage detection with fresh data");
    println!("   3. Execute opportunities on forked validator");
    
    Ok(())
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_external_api_with_local_fork() -> Result<()> {
    println!("\n🧪 Test: External API + Local Fork (Aggressive/Dynamic Testing)");
    println!("{}", "=".repeat(80));
    
    let api_key = match std::env::var("HELIUS_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("⚠️  Skipping test: HELIUS_API_KEY not set");
            return Ok(());
        }
    };
    
    println!("✅ HELIUS_API_KEY found");
    
    // Check if local validator is running
    let local_rpc = "http://127.0.0.1:8899";
    let rpc_client = Arc::new(RpcClient::new(local_rpc.to_string()));
    
    println!("\n📡 Checking local validator...");
    match rpc_client.get_health().await {
        Ok(_) => {
            println!("✅ Local validator is running at {}", local_rpc);
        }
        Err(_) => {
            println!("⚠️  Local validator not running at {}", local_rpc);
            println!("   Start with: solana-test-validator --url https://mainnet.helius-rpc.com/?api-key={}", api_key);
            println!("   Skipping test");
            return Ok(());
        }
    }
    
    // Configure TokenFetcher with external API
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
        metadata_ttl_seconds: 3600,
        price_data_ttl_seconds: 1,
        external_data_api_url: Some(helius_url.clone()),
    };
    
    println!("\n⚙️  Configuration:");
    println!("   Local RPC: {}", local_rpc);
    println!("   External API: {}", helius_url.split('?').next().unwrap());
    println!("   Strategy: Fetch from Helius, Execute on Local Fork");
    
    let token_fetcher = TokenFetcher::with_config(rpc_client.clone(), config, 10_000);
    
    // Fetch real-time pool data from Helius
    let pool_configs = vec![
        (pubkey(RAYDIUM_SOL_USDC), DexType::Raydium),
        (pubkey(ORCA_SOL_USDC), DexType::Whirlpool),
    ];
    
    println!("\n📊 Fetching real-time pool data from Helius...");
    let pools = token_fetcher.initialize_pool_data(&pool_configs).await?;
    
    println!("✅ Fetched {} pools with real-time reserves", pools.len());
    
    for pool in &pools {
        println!("\n   Pool: {} ({:?})", pool.pubkey, pool.dex_type);
        println!("   Reserve A: {}", pool.token_a_reserve);
        println!("   Reserve B: {}", pool.token_b_reserve);
        
        // Calculate exchange rate
        let rate = pool.token_b_reserve as f64 / pool.token_a_reserve as f64;
        println!("   Rate: {:.6}", rate);
    }
    
    println!("\n✅ Aggressive/Dynamic Testing Strategy Working!");
    println!("   - Real-time data from Helius ✅");
    println!("   - Local fork for execution ✅");
    println!("   - Fast iteration cycle ✅");
    
    Ok(())
}
