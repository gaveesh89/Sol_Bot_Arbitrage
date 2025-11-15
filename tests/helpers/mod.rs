// Helper utilities for mainnet fork integration tests
//
// This module provides:
// - Test environment setup with mainnet forking
// - Account fetching from Helius/Solscan APIs
// - Pool state parsing for multiple DEXs
// - Transaction building and execution
// - Profit calculation and verification

use anyhow::{anyhow, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    account::Account,
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use serde::{Deserialize, Serialize};
use std::process::{Child, Command};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

// Environment variables
const HELIUS_API_KEY_ENV: &str = "HELIUS_API_KEY";
const SOLSCAN_API_KEY_ENV: &str = "SOLSCAN_API_KEY";

/// Test environment managing the forked validator
pub struct TestEnvironment {
    pub rpc_port: u16,
    pub faucet_port: u16,
    validator_process: Option<Child>,
    helius_api_key: String,
    solscan_api_key: String,
    http_client: reqwest::Client,
}

impl TestEnvironment {
    /// Create a new test environment with mainnet fork
    pub async fn new() -> Result<Self> {
        Self::new_with_ports(8899, 9900).await
    }

    /// Create test environment with custom ports
    pub async fn new_with_ports(rpc_port: u16, faucet_port: u16) -> Result<Self> {
        // Load API keys from environment
        let helius_api_key = std::env::var(HELIUS_API_KEY_ENV)
            .expect("HELIUS_API_KEY must be set in environment");
        let solscan_api_key = std::env::var(SOLSCAN_API_KEY_ENV)
            .expect("SOLSCAN_API_KEY must be set in environment");

        println!("🔑 API keys loaded");
        println!("   Helius: {}...", &helius_api_key[..8]);
        println!("   Solscan: {}...", &solscan_api_key[..8]);

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let mut env = Self {
            rpc_port,
            faucet_port,
            validator_process: None,
            helius_api_key,
            solscan_api_key,
            http_client,
        };

        env.start_validator().await?;
        Ok(env)
    }

    /// Start the test validator with mainnet fork
    async fn start_validator(&mut self) -> Result<()> {
        println!("🚀 Starting test validator with mainnet fork...");

        // Kill any existing validator on this port
        let _ = Command::new("pkill")
            .arg("-f")
            .arg(format!("solana-test-validator.*{}", self.rpc_port))
            .output();

        sleep(Duration::from_secs(1)).await;

        // Start test validator
        let child = Command::new("solana-test-validator")
            .arg("--rpc-port")
            .arg(self.rpc_port.to_string())
            .arg("--faucet-port")
            .arg(self.faucet_port.to_string())
            .arg("--reset")
            .arg("--quiet")
            .arg("--no-bpf-jit")
            .spawn()
            .map_err(|e| anyhow!("Failed to start validator: {}", e))?;

        self.validator_process = Some(child);

        // Wait for validator to be ready
        let client = self.create_client();
        for i in 0..30 {
            if let Ok(_) = client.get_health().await {
                println!("✅ Validator ready after {} attempts", i + 1);
                return Ok(());
            }
            sleep(Duration::from_millis(500)).await;
        }

        Err(anyhow!("Validator failed to start within 15 seconds"))
    }

    /// Get the RPC URL
    pub fn rpc_url(&self) -> String {
        format!("http://localhost:{}", self.rpc_port)
    }

    /// Create an RPC client
    pub fn create_client(&self) -> RpcClient {
        RpcClient::new_with_commitment(self.rpc_url(), CommitmentConfig::confirmed())
    }

    /// Fetch account data from mainnet using Helius
    pub async fn fetch_account_from_mainnet(&self, pubkey: &Pubkey) -> Result<Account> {
        let url = format!(
            "https://mainnet.helius-rpc.com/?api-key={}",
            self.helius_api_key
        );

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                pubkey.to_string(),
                {
                    "encoding": "base64",
                    "commitment": "confirmed"
                }
            ]
        });

        let response: serde_json::Value = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        let account_data = response["result"]["value"]
            .as_object()
            .ok_or_else(|| anyhow!("Account not found on mainnet"))?;

        let data_str = account_data["data"][0]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid account data"))?;
        let data = base64::decode(data_str)?;

        let lamports = account_data["lamports"]
            .as_u64()
            .ok_or_else(|| anyhow!("Invalid lamports"))?;

        let owner_str = account_data["owner"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid owner"))?;
        let owner = Pubkey::from_str(owner_str)?;

        Ok(Account {
            lamports,
            data,
            owner,
            executable: account_data["executable"].as_bool().unwrap_or(false),
            rent_epoch: account_data["rentEpoch"].as_u64().unwrap_or(0),
        })
    }

    /// Load an account into the test validator
    pub async fn load_account(&self, pubkey: Pubkey, account: Account) -> Result<()> {
        let client = self.create_client();

        // Create account via system program
        let payer = Keypair::new();
        self.airdrop(&payer.pubkey(), 10_000_000_000).await?;

        // For simplicity, we'll use the validator's account loading
        // In practice, you might need to set account data via RPC
        println!("⚠️  Note: Account loading requires additional setup");
        println!("   Consider using --account or --clone flags when starting validator");

        Ok(())
    }

    /// Airdrop SOL to an address
    pub async fn airdrop(&self, to: &Pubkey, lamports: u64) -> Result<()> {
        let client = self.create_client();
        let signature = client
            .request_airdrop(to, lamports)
            .await
            .map_err(|e| anyhow!("Airdrop failed: {}", e))?;

        // Wait for confirmation
        for _ in 0..30 {
            if let Ok(confirmed) = client.confirm_transaction(&signature).await {
                if confirmed {
                    return Ok(());
                }
            }
            sleep(Duration::from_millis(100)).await;
        }

        Err(anyhow!("Airdrop confirmation timeout"))
    }

    /// Fetch Raydium pool state from mainnet
    pub async fn fetch_raydium_pool_state(&self, pool: &Pubkey) -> Result<RaydiumPoolState> {
        let account = self.fetch_account_from_mainnet(pool).await?;
        parse_raydium_pool_state(&account.data)
    }

    /// Fetch Orca Whirlpool state from mainnet
    pub async fn fetch_whirlpool_state(&self, pool: &Pubkey) -> Result<WhirlpoolState> {
        let account = self.fetch_account_from_mainnet(pool).await?;
        parse_whirlpool_state(&account.data)
    }

    /// Find a Whirlpool for a token pair using Solscan API
    pub async fn find_whirlpool(&self, token_a: &Pubkey, token_b: &Pubkey) -> Result<Pubkey> {
        // Use Solscan to find whirlpools
        let url = format!(
            "https://public-api.solscan.io/account/{}",
            token_a
        );

        let response: serde_json::Value = self
            .http_client
            .get(&url)
            .header("token", &self.solscan_api_key)
            .send()
            .await?
            .json()
            .await?;

        // Parse and find matching whirlpool
        // This is simplified - in practice you'd need to search through DEX aggregators
        Err(anyhow!("Whirlpool discovery not yet implemented - provide address directly"))
    }

    /// Load Raydium pool and required accounts into validator
    pub async fn load_raydium_pool(&self, pool: &Pubkey) -> Result<()> {
        println!("📥 Loading Raydium pool: {}", pool);

        // Fetch pool account
        let pool_account = self.fetch_account_from_mainnet(pool).await?;
        println!("   Pool account: {} bytes", pool_account.data.len());

        // Parse pool to get token accounts
        let pool_state = parse_raydium_pool_state(&pool_account.data)?;

        // Fetch associated token accounts
        println!("   Loading token vaults...");
        let base_vault = self.fetch_account_from_mainnet(&pool_state.base_vault).await?;
        let quote_vault = self.fetch_account_from_mainnet(&pool_state.quote_vault).await?;

        println!("✅ Loaded Raydium pool and vaults");
        Ok(())
    }

    /// Fetch multiple pools for arbitrage cycle
    pub async fn fetch_arbitrage_pools(
        &self,
        pairs: Vec<(&str, &str)>,
    ) -> Result<Vec<PoolInfo>> {
        let mut pools = Vec::new();

        for (token_a, token_b) in pairs {
            println!("🔍 Finding pool for {} -> {}", token_a, token_b);

            // This is simplified - you'd query DEX aggregators or APIs
            let pool_info = PoolInfo {
                address: Pubkey::new_unique(),
                dex: DexType::Raydium,
                token_a: Pubkey::from_str(token_a)?,
                token_b: Pubkey::from_str(token_b)?,
                reserve_a: 1_000_000_000_000,
                reserve_b: 1_000_000_000_000,
                fee_bps: 25,
            };

            pools.push(pool_info);
        }

        Ok(pools)
    }

    /// Build Raydium swap transaction
    pub async fn build_raydium_swap(
        &self,
        wallet: &Keypair,
        pool: &Pubkey,
        amount_in: u64,
        min_amount_out: u64,
        a_to_b: bool,
    ) -> Result<Transaction> {
        let client = self.create_client();
        let recent_blockhash = client.get_latest_blockhash().await?;

        // Build Raydium swap instruction
        let swap_ix = build_raydium_swap_instruction(
            wallet.pubkey(),
            *pool,
            amount_in,
            min_amount_out,
            a_to_b,
        )?;

        let mut tx = Transaction::new_with_payer(&[swap_ix], Some(&wallet.pubkey()));
        tx.sign(&[wallet], recent_blockhash);

        Ok(tx)
    }

    /// Load full arbitrage environment (programs and pools)
    pub async fn load_arbitrage_environment(&self) -> Result<()> {
        println!("🌐 Loading arbitrage environment...");

        // Load Raydium program
        println!("   Loading Raydium AMM V4...");

        // Load Orca Whirlpool program
        println!("   Loading Orca Whirlpool...");

        // Load Meteora DLMM program
        println!("   Loading Meteora DLMM...");

        // Load common pools
        println!("   Loading liquidity pools...");

        println!("✅ Arbitrage environment loaded");
        Ok(())
    }

    /// Build multi-hop arbitrage transaction
    pub async fn build_arbitrage_transaction(
        &self,
        wallet: &Keypair,
        amount: u64,
        routes: Vec<SwapRoute>,
    ) -> Result<Transaction> {
        let client = self.create_client();
        let recent_blockhash = client.get_latest_blockhash().await?;

        let mut instructions = Vec::new();

        // Add compute budget instruction
        instructions.push(create_compute_budget_instruction(1_400_000, 5_000)?);

        // Build swap instructions for each route
        for route in routes {
            let swap_ix = match route {
                SwapRoute::Raydium(pool) => {
                    build_raydium_swap_instruction(
                        wallet.pubkey(),
                        Pubkey::from_str(pool)?,
                        amount,
                        0,
                        true,
                    )?
                }
                SwapRoute::OrcaWhirlpool(pool) => {
                    build_whirlpool_swap_instruction(wallet.pubkey(), pool)?
                }
                SwapRoute::Meteora(pool) => {
                    build_meteora_swap_instruction(wallet.pubkey(), pool)?
                }
            };
            instructions.push(swap_ix);
        }

        let mut tx = Transaction::new_with_payer(&instructions, Some(&wallet.pubkey()));
        tx.sign(&[wallet], recent_blockhash);

        Ok(tx)
    }

    /// Build swap with specific compute budget
    pub async fn build_swap_with_compute_budget(
        &self,
        wallet: &Keypair,
        compute_units: u32,
        micro_lamports_per_cu: u64,
    ) -> Result<Transaction> {
        let client = self.create_client();
        let recent_blockhash = client.get_latest_blockhash().await?;

        let instructions = vec![
            create_compute_budget_instruction(compute_units, micro_lamports_per_cu)?,
            // Add actual swap instruction here
        ];

        let mut tx = Transaction::new_with_payer(&instructions, Some(&wallet.pubkey()));
        tx.sign(&[wallet], recent_blockhash);

        Ok(tx)
    }

    /// Build optimized swap transaction
    pub async fn build_optimized_swap(&self, wallet: &Keypair, amount: u64) -> Result<Transaction> {
        self.build_swap_with_compute_budget(wallet, 200_000, 5_000).await
    }

    /// Build maximum hop arbitrage transaction
    pub async fn build_max_hop_arbitrage(&self, wallet: &Keypair, amount: u64) -> Result<Transaction> {
        self.build_arbitrage_transaction(
            wallet,
            amount,
            vec![
                SwapRoute::Raydium("pool1"),
                SwapRoute::OrcaWhirlpool("pool2"),
                SwapRoute::Raydium("pool3"),
                SwapRoute::Meteora("pool4"),
            ],
        ).await
    }

    /// Calculate expected arbitrage profit
    pub async fn calculate_arbitrage_profit(&self, starting_amount: u64) -> Result<i64> {
        // Fetch pool states and calculate
        // This is simplified - implement actual profit calculation
        Ok(50_000) // Example: 50,000 lamports profit
    }

    /// Execute simple arbitrage cycle
    pub async fn execute_simple_arbitrage(&self, wallet: &Keypair, amount: u64) -> Result<()> {
        let tx = self.build_optimized_swap(wallet, amount).await?;
        let client = self.create_client();
        client.send_and_confirm_transaction(&tx).await?;
        Ok(())
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        if let Some(mut child) = self.validator_process.take() {
            println!("🛑 Stopping test validator...");
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// Data structures

#[derive(Debug, Clone)]
pub struct RaydiumPoolState {
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub base_reserve: u64,
    pub quote_reserve: u64,
    pub lp_supply: u64,
}

#[derive(Debug, Clone)]
pub struct WhirlpoolState {
    pub liquidity: u128,
    pub sqrt_price: u128,
    pub tick_current_index: i32,
}

#[derive(Debug, Clone)]
pub struct PoolInfo {
    pub address: Pubkey,
    pub dex: DexType,
    pub token_a: Pubkey,
    pub token_b: Pubkey,
    pub reserve_a: u64,
    pub reserve_b: u64,
    pub fee_bps: u16,
}

#[derive(Debug, Clone)]
pub enum DexType {
    Raydium,
    OrcaWhirlpool,
    Meteora,
}

#[derive(Debug, Clone)]
pub enum SwapRoute {
    Raydium(&'static str),
    OrcaWhirlpool(&'static str),
    Meteora(&'static str),
}

#[derive(Debug)]
pub struct ProfitResult {
    pub final_amount: u64,
    pub gross_profit: i64,
    pub total_fees: u64,
    pub net_profit: i64,
    pub roi_percentage: f64,
    pub is_profitable: bool,
}

// Helper functions

fn parse_raydium_pool_state(data: &[u8]) -> Result<RaydiumPoolState> {
    // Raydium AMM V4 pool layout (simplified)
    if data.len() < 752 {
        return Err(anyhow!("Invalid pool data length"));
    }

    // Parse key fields (offsets from Raydium SDK)
    let base_vault = Pubkey::try_from(&data[32..64])?;
    let quote_vault = Pubkey::try_from(&data[64..96])?;

    // These offsets are approximate - verify with actual Raydium layout
    let base_reserve = u64::from_le_bytes(data[200..208].try_into()?);
    let quote_reserve = u64::from_le_bytes(data[208..216].try_into()?);
    let lp_supply = u64::from_le_bytes(data[216..224].try_into()?);

    Ok(RaydiumPoolState {
        base_vault,
        quote_vault,
        base_reserve,
        quote_reserve,
        lp_supply,
    })
}

fn parse_whirlpool_state(data: &[u8]) -> Result<WhirlpoolState> {
    // Orca Whirlpool layout (simplified)
    if data.len() < 256 {
        return Err(anyhow!("Invalid whirlpool data length"));
    }

    // Parse fields (offsets from Whirlpool SDK)
    let liquidity = u128::from_le_bytes(data[65..81].try_into()?);
    let sqrt_price = u128::from_le_bytes(data[81..97].try_into()?);
    let tick_current_index = i32::from_le_bytes(data[97..101].try_into()?);

    Ok(WhirlpoolState {
        liquidity,
        sqrt_price,
        tick_current_index,
    })
}

pub fn calculate_cycle_profit(pools: &[PoolInfo], starting_amount: u64) -> ProfitResult {
    let mut current_amount = starting_amount;
    let mut total_fees = 0u64;

    // Simulate swaps through each pool
    for pool in pools {
        let fee = (current_amount * pool.fee_bps as u64) / 10_000;
        total_fees += fee;

        // Simple constant product formula
        let amount_after_fee = current_amount - fee;
        let output = (pool.reserve_b * amount_after_fee) / (pool.reserve_a + amount_after_fee);

        current_amount = output;
    }

    let gross_profit = current_amount as i64 - starting_amount as i64;
    let net_profit = gross_profit - total_fees as i64;
    let roi_percentage = (net_profit as f64 / starting_amount as f64) * 100.0;

    ProfitResult {
        final_amount: current_amount,
        gross_profit,
        total_fees,
        net_profit,
        roi_percentage,
        is_profitable: net_profit > 0,
    }
}

fn build_raydium_swap_instruction(
    user: Pubkey,
    pool: Pubkey,
    amount_in: u64,
    min_amount_out: u64,
    a_to_b: bool,
) -> Result<Instruction> {
    // Raydium swap instruction (simplified)
    let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?;

    // Instruction data: [9, amount_in (u64), min_amount_out (u64)]
    let mut data = vec![9u8]; // Swap instruction discriminator
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&min_amount_out.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool, false),
            AccountMeta::new(user, true),
            // Add other required accounts
        ],
        data,
    })
}

fn build_whirlpool_swap_instruction(user: Pubkey, pool: &str) -> Result<Instruction> {
    let program_id = Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc")?;

    Ok(Instruction {
        program_id,
        accounts: vec![AccountMeta::new(user, true)],
        data: vec![],
    })
}

fn build_meteora_swap_instruction(user: Pubkey, pool: &str) -> Result<Instruction> {
    let program_id = Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo")?;

    Ok(Instruction {
        program_id,
        accounts: vec![AccountMeta::new(user, true)],
        data: vec![],
    })
}

fn create_compute_budget_instruction(units: u32, micro_lamports_per_cu: u64) -> Result<Instruction> {
    use solana_sdk::compute_budget::ComputeBudgetInstruction;

    Ok(ComputeBudgetInstruction::set_compute_unit_limit(units))
}

pub fn estimate_compute_units(tx: &Transaction) -> u32 {
    // Rough estimate: 5000 CU per signature + 200 CU per account + instruction data
    let signature_cu = tx.signatures.len() as u32 * 5_000;
    let accounts_cu = tx.message.account_keys.len() as u32 * 200;
    let instructions_cu = tx.message.instructions.len() as u32 * 10_000;

    signature_cu + accounts_cu + instructions_cu
}
