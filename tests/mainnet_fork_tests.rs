// Mainnet Fork Integration Tests for Arbitrage Detection and Execution
//
// This test suite forks Solana mainnet state, fetches real pool data,
// and executes multi-hop swaps to verify arbitrage opportunities.
//
// Prerequisites:
// 1. solana-test-validator installed
// 2. Helius API key set in .env
// 3. Solscan API key set in .env
//
// Run with: cargo test --test mainnet_fork_tests -- --test-threads=1 --nocapture

mod helpers;

use helpers::*;
use serial_test::serial;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;
use std::time::Duration;

// Known Program IDs
const RAYDIUM_AMM_V4: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const RAYDIUM_USDC_SOL_POOL: &str = "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2";
const ORCA_WHIRLPOOL_PROGRAM: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
const METEORA_DLMM_PROGRAM: &str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";

// Token Mints
const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

#[tokio::test]
#[serial]
#[ignore] // Remove this to run the test
async fn test_fork_mainnet_and_fetch_pools() {
    println!("🚀 Starting mainnet fork test...");
    
    // Initialize test environment
    let test_env = TestEnvironment::new().await.expect("Failed to setup test environment");
    
    println!("✅ Test validator started on port {}", test_env.rpc_port);
    println!("📍 RPC URL: {}", test_env.rpc_url());
    
    // Verify we can connect
    let client = test_env.create_client();
    let slot = client.get_slot().await.expect("Failed to get slot");
    println!("✅ Connected to validator at slot: {}", slot);
    
    // Fetch Raydium pool state
    let pool_pubkey = Pubkey::from_str(RAYDIUM_USDC_SOL_POOL).unwrap();
    let pool_account = test_env
        .fetch_account_from_mainnet(&pool_pubkey)
        .await
        .expect("Failed to fetch pool account");
    
    println!("✅ Fetched Raydium pool account: {} bytes", pool_account.data.len());
    
    // Load pool into test validator
    test_env
        .load_account(pool_pubkey, pool_account)
        .await
        .expect("Failed to load account");
    
    println!("✅ Loaded pool account into test validator");
    
    // Verify account exists in test validator
    let loaded_account = client
        .get_account(&pool_pubkey)
        .await
        .expect("Failed to get loaded account");
    
    assert!(!loaded_account.data.is_empty(), "Account data should not be empty");
    println!("✅ Verified pool account exists in test validator");
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_fetch_raydium_pool_data() {
    println!("🔍 Fetching Raydium pool data from mainnet...");
    
    let test_env = TestEnvironment::new().await.unwrap();
    
    // Fetch pool state using Helius
    let pool_pubkey = Pubkey::from_str(RAYDIUM_USDC_SOL_POOL).unwrap();
    let pool_data = test_env
        .fetch_raydium_pool_state(&pool_pubkey)
        .await
        .expect("Failed to fetch Raydium pool state");
    
    println!("✅ Pool reserves:");
    println!("   Base (SOL): {}", pool_data.base_reserve);
    println!("   Quote (USDC): {}", pool_data.quote_reserve);
    println!("   LP Supply: {}", pool_data.lp_supply);
    
    assert!(pool_data.base_reserve > 0, "Base reserve should be non-zero");
    assert!(pool_data.quote_reserve > 0, "Quote reserve should be non-zero");
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_fetch_orca_whirlpool_data() {
    println!("🔍 Fetching Orca Whirlpool data from mainnet...");
    
    let test_env = TestEnvironment::new().await.unwrap();
    
    // Find a popular whirlpool (SOL/USDC)
    let whirlpool_address = test_env
        .find_whirlpool(
            &Pubkey::from_str(SOL_MINT).unwrap(),
            &Pubkey::from_str(USDC_MINT).unwrap(),
        )
        .await
        .expect("Failed to find whirlpool");
    
    println!("✅ Found Whirlpool: {}", whirlpool_address);
    
    let pool_data = test_env
        .fetch_whirlpool_state(&whirlpool_address)
        .await
        .expect("Failed to fetch whirlpool state");
    
    println!("✅ Whirlpool data:");
    println!("   Liquidity: {}", pool_data.liquidity);
    println!("   Current tick: {}", pool_data.tick_current_index);
    println!("   Sqrt price: {}", pool_data.sqrt_price);
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_detect_triangular_arbitrage_opportunity() {
    println!("🎯 Detecting triangular arbitrage opportunities...");
    
    let test_env = TestEnvironment::new().await.unwrap();
    
    // Fetch pools for SOL -> USDC -> USDT -> SOL cycle
    let pools = test_env
        .fetch_arbitrage_pools(vec![
            (SOL_MINT, USDC_MINT),
            (USDC_MINT, USDT_MINT),
            (USDT_MINT, SOL_MINT),
        ])
        .await
        .expect("Failed to fetch pools");
    
    println!("✅ Fetched {} pools for arbitrage cycle", pools.len());
    
    // Calculate expected profit
    let starting_amount = 1_000_000_000; // 1 SOL
    let profit_result = calculate_cycle_profit(&pools, starting_amount);
    
    println!("💰 Arbitrage analysis:");
    println!("   Starting amount: {} lamports", starting_amount);
    println!("   Expected return: {} lamports", profit_result.final_amount);
    println!("   Gross profit: {} lamports", profit_result.gross_profit);
    println!("   Fees paid: {} lamports", profit_result.total_fees);
    println!("   Net profit: {} lamports", profit_result.net_profit);
    println!("   ROI: {:.4}%", profit_result.roi_percentage);
    
    if profit_result.is_profitable {
        println!("✅ Profitable opportunity detected!");
    } else {
        println!("⚠️  Not profitable after fees");
    }
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_execute_swap_on_raydium() {
    println!("🔄 Executing swap on Raydium (forked mainnet)...");
    
    let test_env = TestEnvironment::new().await.unwrap();
    let client = test_env.create_client();
    
    // Create a funded test wallet
    let wallet = Keypair::new();
    let airdrop_amount = 10_000_000_000; // 10 SOL
    
    test_env
        .airdrop(&wallet.pubkey(), airdrop_amount)
        .await
        .expect("Failed to airdrop");
    
    println!("✅ Funded wallet: {}", wallet.pubkey());
    
    // Load Raydium program and pool
    let pool_pubkey = Pubkey::from_str(RAYDIUM_USDC_SOL_POOL).unwrap();
    test_env
        .load_raydium_pool(&pool_pubkey)
        .await
        .expect("Failed to load Raydium pool");
    
    // Build swap transaction
    let swap_amount = 1_000_000_000; // 1 SOL
    let min_amount_out = 0; // Accept any amount for testing
    
    let swap_tx = test_env
        .build_raydium_swap(
            &wallet,
            &pool_pubkey,
            swap_amount,
            min_amount_out,
            true, // SOL -> USDC
        )
        .await
        .expect("Failed to build swap transaction");
    
    println!("✅ Built swap transaction: {} bytes", swap_tx.message().serialize().len());
    
    // Verify transaction size is within limits
    let tx_size = swap_tx.message().serialize().len();
    assert!(tx_size <= 1232, "Transaction too large: {} bytes", tx_size);
    
    // Execute transaction
    let signature = client
        .send_and_confirm_transaction(&swap_tx)
        .await
        .expect("Failed to execute swap");
    
    println!("✅ Swap executed successfully!");
    println!("   Signature: {}", signature);
    
    // Verify balances changed
    let final_balance = client
        .get_balance(&wallet.pubkey())
        .await
        .expect("Failed to get balance");
    
    println!("   Final balance: {} lamports", final_balance);
    assert!(final_balance < airdrop_amount, "Balance should have decreased");
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_execute_triangular_arbitrage() {
    println!("🎯 Executing full triangular arbitrage cycle...");
    
    let test_env = TestEnvironment::new().await.unwrap();
    let client = test_env.create_client();
    
    // Create funded wallet
    let wallet = Keypair::new();
    test_env
        .airdrop(&wallet.pubkey(), 10_000_000_000)
        .await
        .expect("Failed to airdrop");
    
    let initial_balance = client
        .get_balance(&wallet.pubkey())
        .await
        .unwrap();
    
    println!("💰 Initial balance: {} lamports", initial_balance);
    
    // Load all required pools and programs
    test_env
        .load_arbitrage_environment()
        .await
        .expect("Failed to load arbitrage environment");
    
    // Build multi-hop swap transaction
    let starting_amount = 1_000_000_000; // 1 SOL
    let arbitrage_tx = test_env
        .build_arbitrage_transaction(
            &wallet,
            starting_amount,
            vec![
                SwapRoute::Raydium(RAYDIUM_USDC_SOL_POOL),
                SwapRoute::OrcaWhirlpool("whirlpool_usdc_usdt"),
                SwapRoute::Raydium("raydium_usdt_sol"),
            ],
        )
        .await
        .expect("Failed to build arbitrage transaction");
    
    // Verify compute budget
    let compute_units = estimate_compute_units(&arbitrage_tx);
    println!("🖥️  Estimated compute units: {}", compute_units);
    assert!(compute_units <= 1_400_000, "Exceeds compute limit");
    
    // Verify transaction size
    let tx_size = arbitrage_tx.message().serialize().len();
    println!("📦 Transaction size: {} bytes", tx_size);
    assert!(tx_size <= 1232, "Transaction too large");
    
    // Execute arbitrage
    let signature = client
        .send_and_confirm_transaction(&arbitrage_tx)
        .await
        .expect("Failed to execute arbitrage");
    
    println!("✅ Arbitrage executed!");
    println!("   Signature: {}", signature);
    
    // Check final balance
    let final_balance = client
        .get_balance(&wallet.pubkey())
        .await
        .unwrap();
    
    let profit = final_balance as i64 - initial_balance as i64;
    println!("💰 Final balance: {} lamports", final_balance);
    println!("📊 Net profit: {} lamports", profit);
    
    if profit > 0 {
        println!("✅ PROFITABLE! Gained {} lamports", profit);
    } else {
        println!("⚠️  Lost {} lamports (fees + slippage)", profit.abs());
    }
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_compute_budget_optimization() {
    println!("🖥️  Testing compute budget optimization...");
    
    let test_env = TestEnvironment::new().await.unwrap();
    
    // Test with different compute unit limits
    let test_cases = vec![
        (200_000, "Single swap"),
        (400_000, "Two swaps"),
        (800_000, "Three swaps"),
        (1_400_000, "Maximum allowed"),
    ];
    
    for (compute_limit, description) in test_cases {
        println!("\n📊 Testing: {} ({} CU)", description, compute_limit);
        
        let wallet = Keypair::new();
        test_env.airdrop(&wallet.pubkey(), 5_000_000_000).await.unwrap();
        
        let tx = test_env
            .build_swap_with_compute_budget(
                &wallet,
                compute_limit,
                5_000, // microlamports per CU
            )
            .await
            .expect("Failed to build transaction");
        
        let actual_cu = estimate_compute_units(&tx);
        println!("   Requested: {} CU", compute_limit);
        println!("   Actual: {} CU", actual_cu);
        println!("   Efficiency: {:.2}%", (actual_cu as f64 / compute_limit as f64) * 100.0);
    }
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_transaction_size_optimization() {
    println!("📦 Testing transaction size optimization...");
    
    let test_env = TestEnvironment::new().await.unwrap();
    let wallet = Keypair::new();
    
    // Build transaction with minimal data
    let minimal_tx = test_env
        .build_optimized_swap(&wallet, 1_000_000_000)
        .await
        .expect("Failed to build minimal transaction");
    
    let minimal_size = minimal_tx.message().serialize().len();
    println!("📏 Minimal transaction: {} bytes", minimal_size);
    
    // Build transaction with maximum swaps
    let max_swaps_tx = test_env
        .build_max_hop_arbitrage(&wallet, 1_000_000_000)
        .await
        .expect("Failed to build max hop transaction");
    
    let max_size = max_swaps_tx.message().serialize().len();
    println!("📏 Max hops transaction: {} bytes", max_size);
    println!("📊 Remaining capacity: {} bytes", 1232 - max_size);
    
    assert!(max_size <= 1232, "Transaction exceeds size limit");
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_profit_verification() {
    println!("💰 Testing profit verification accuracy...");
    
    let test_env = TestEnvironment::new().await.unwrap();
    let client = test_env.create_client();
    
    let wallet = Keypair::new();
    test_env.airdrop(&wallet.pubkey(), 10_000_000_000).await.unwrap();
    
    // Get initial balance
    let balance_before = client.get_balance(&wallet.pubkey()).await.unwrap();
    
    // Calculate expected profit
    let expected_profit = test_env
        .calculate_arbitrage_profit(1_000_000_000)
        .await
        .expect("Failed to calculate expected profit");
    
    println!("📈 Expected profit: {} lamports", expected_profit);
    
    // Execute arbitrage
    test_env
        .execute_simple_arbitrage(&wallet, 1_000_000_000)
        .await
        .expect("Failed to execute arbitrage");
    
    // Get final balance
    let balance_after = client.get_balance(&wallet.pubkey()).await.unwrap();
    let actual_profit = balance_after as i64 - balance_before as i64;
    
    println!("📊 Actual profit: {} lamports", actual_profit);
    
    let difference = (actual_profit - expected_profit).abs();
    let tolerance = expected_profit / 100; // 1% tolerance
    
    println!("🎯 Accuracy:");
    println!("   Expected: {} lamports", expected_profit);
    println!("   Actual: {} lamports", actual_profit);
    println!("   Difference: {} lamports", difference);
    println!("   Tolerance: {} lamports", tolerance);
    
    assert!(
        difference <= tolerance,
        "Profit verification failed: difference {} exceeds tolerance {}",
        difference,
        tolerance
    );
}
