/// Test to find correct Raydium AMM V4 offsets
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use reqwest;
use base64::{Engine as _, engine::general_purpose};
use serial_test::serial;

const RAYDIUM_SOL_USDC: &str = "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2";

// Known values from Solscan
const EXPECTED_BASE_VAULT: &str = "DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz";
const EXPECTED_QUOTE_VAULT: &str = "HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz";
const EXPECTED_BASE_MINT: &str = "So11111111111111111111111111111111111111112";
const EXPECTED_QUOTE_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const EXPECTED_LP_MINT: &str = "8HoQnePLqPj4M7PUDzfw8e3Ymdwgc7NLGnaTUapubyvu";

#[tokio::test]
#[serial]
#[ignore]
async fn test_find_raydium_offsets() -> Result<()> {
    println!("\n🔍 Finding Raydium AMM V4 Offsets");
    
    let api_key = match std::env::var("HELIUS_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("⚠️  Skipping: HELIUS_API_KEY not set");
            return Ok(());
        }
    };
    
    // Fetch pool account
    let url = format!("https://mainnet.helius-rpc.com/?api-key={}", api_key);
    let client = reqwest::Client::new();
    
    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [RAYDIUM_SOL_USDC, {"encoding": "base64"}]
        }))
        .send()
        .await?;
    
    let json: serde_json::Value = response.json().await?;
    let data_b64 = json["result"]["value"]["data"][0]
        .as_str()
        .expect("Missing data");
    let data = general_purpose::STANDARD.decode(data_b64)?;
    
    println!("Account data length: {} bytes", data.len());
    println!();
    
    // Expected values
    let expected_base_vault = Pubkey::from_str(EXPECTED_BASE_VAULT)?;
    let expected_quote_vault = Pubkey::from_str(EXPECTED_QUOTE_VAULT)?;
    let expected_base_mint = Pubkey::from_str(EXPECTED_BASE_MINT)?;
    let expected_quote_mint = Pubkey::from_str(EXPECTED_QUOTE_MINT)?;
    let expected_lp_mint = Pubkey::from_str(EXPECTED_LP_MINT)?;
    
    // Search for each pubkey
    fn find_pubkey(data: &[u8], target: &Pubkey) -> Option<usize> {
        let target_bytes = target.to_bytes();
        for i in 0..=(data.len() - 32) {
            if &data[i..i+32] == target_bytes.as_ref() {
                return Some(i);
            }
        }
        None
    }
    
    println!("Searching for pubkeys...\n");
    
    if let Some(offset) = find_pubkey(&data, &expected_base_vault) {
        println!("✅ Base Vault found at offset: {}", offset);
        println!("   Address: {}", EXPECTED_BASE_VAULT);
    } else {
        println!("❌ Base Vault NOT FOUND");
    }
    
    if let Some(offset) = find_pubkey(&data, &expected_quote_vault) {
        println!("✅ Quote Vault found at offset: {}", offset);
        println!("   Address: {}", EXPECTED_QUOTE_VAULT);
    } else {
        println!("❌ Quote Vault NOT FOUND");
    }
    
    if let Some(offset) = find_pubkey(&data, &expected_base_mint) {
        println!("✅ Base Mint found at offset: {}", offset);
        println!("   Address: {}", EXPECTED_BASE_MINT);
    } else {
        println!("❌ Base Mint NOT FOUND");
    }
    
    if let Some(offset) = find_pubkey(&data, &expected_quote_mint) {
        println!("✅ Quote Mint found at offset: {}", offset);
        println!("   Address: {}", EXPECTED_QUOTE_MINT);
    } else {
        println!("❌ Quote Mint NOT FOUND");
    }
    
    if let Some(offset) = find_pubkey(&data, &expected_lp_mint) {
        println!("✅ LP Mint found at offset: {}", offset);
        println!("   Address: {}", EXPECTED_LP_MINT);
    } else {
        println!("❌ LP Mint NOT FOUND");
    }
    
    Ok(())
}
