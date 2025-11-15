/// Debug test to verify pool parsing works correctly
use anyhow::Result;
use solana_mev_bot::chain::token_fetch::{TokenFetcher, TokenFetchConfig, DexType};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use solana_client::nonblocking::rpc_client::RpcClient;
use serial_test::serial;

const RAYDIUM_SOL_USDC: &str = "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2";

fn pubkey(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_pool_parsing_debug() -> Result<()> {
    println!("\n🐛 Debug: Pool Parsing Test");
    
    let api_key = match std::env::var("HELIUS_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("⚠️  Skipping: HELIUS_API_KEY not set");
            return Ok(());
        }
    };
    
    let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
    let rpc_client = Arc::new(RpcClient::new(rpc_url));
    
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
    
    let token_fetcher = TokenFetcher::with_config(rpc_client, config, 10_000);
    
    let pool_configs = vec![
        (pubkey(RAYDIUM_SOL_USDC), DexType::Raydium),
    ];
    
    println!("Fetching pool: {}", RAYDIUM_SOL_USDC);
    let pools = token_fetcher.initialize_pool_data(&pool_configs).await?;
    
    assert_eq!(pools.len(), 1, "Should fetch 1 pool");
    
    let pool = &pools[0];
    println!("\n📊 Pool Data:");
    println!("  Address: {}", pool.pubkey);
    println!("  DEX: {:?}", pool.dex_type);
    println!("  Token A Mint: {}", pool.token_a_mint);
    println!("  Token B Mint: {}", pool.token_b_mint);
    println!("  Token A Vault: {:?}", pool.token_a_vault);
    println!("  Token B Vault: {:?}", pool.token_b_vault);
    println!("  Token A Reserve: {}", pool.token_a_reserve);
    println!("  Token B Reserve: {}", pool.token_b_reserve);
    println!("  Fee: {}/{}", pool.fee_numerator, pool.fee_denominator);
    
    // Check that vaults are set
    assert!(pool.token_a_vault.is_some(), "Token A vault should be set");
    assert!(pool.token_b_vault.is_some(), "Token B vault should be set");
    
    println!("\n✅ Vaults are set correctly");
    
    // Check if reserves are fetched
    if pool.token_a_reserve > 0 && pool.token_b_reserve > 0 {
        println!("✅ Reserves fetched successfully");
        println!("   Exchange rate: {} Token B per Token A", 
                 pool.token_b_reserve as f64 / pool.token_a_reserve as f64);
    } else {
        println!("⚠️  Reserves are zero - vault enrichment may have failed");
        println!("   This could be:");
        println!("   1. Vaults exist but balances couldn't be fetched");
        println!("   2. Pool is empty/inactive");
        println!("   3. Parsing offset error");
    }
    
    Ok(())
}
