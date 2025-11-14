// Efficient Pool Data Fetcher for Multiple Solana DEXs
//
// This module implements batch fetching of pool accounts from multiple DEXs
// with intelligent caching, retry logic, and multi-RPC failover support.
//
// Supported DEXs:
// - Raydium AMM v4
// - Meteora DAMM (Dynamic AMM)
// - Meteora Vault
// - Orca Whirlpool (v2)
// - Orca v1 (legacy)

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcAccountInfoConfig;
use solana_sdk::{account::Account, commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{debug, warn, info, error};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::dex::triangular_arb::DexType;

// Solana RPC batch size limit
const MAX_BATCH_SIZE: usize = 100;

// DEX Program IDs
const RAYDIUM_AMM_V4: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const METEORA_DAMM: &str = "Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB";
const METEORA_VAULT: &str = "24Uqj9JCLxUeoC3hGfh5W3s9FM9uCHDS2SG3LYwBpyTi";
const ORCA_WHIRLPOOL: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
const ORCA_V1: &str = "9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP";

/// Cached pool data with timestamp
#[derive(Clone, Debug)]
pub struct CachedPoolData {
    pub data: PoolData,
    pub timestamp: i64,
}

impl CachedPoolData {
    /// Check if cache entry is still valid
    pub fn is_valid(&self, ttl_ms: u64) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        
        (current_time - self.timestamp) < ttl_ms as i64
    }
}

/// Pool data structure with reserves and fees
#[derive(Clone, Debug)]
pub struct PoolData {
    pub pool_address: Pubkey,
    pub token_a: Pubkey,
    pub token_b: Pubkey,
    pub reserve_a: u64,
    pub reserve_b: u64,
    pub fee_bps: u16,
    pub dex_type: DexType,
    pub program_id: Pubkey,
}

impl PoolData {
    /// Calculate exchange rate from token A to token B
    pub fn calculate_rate_a_to_b(&self) -> f64 {
        if self.reserve_a == 0 {
            return 0.0;
        }
        
        let rate = self.reserve_b as f64 / self.reserve_a as f64;
        let fee_multiplier = 1.0 - (self.fee_bps as f64 / 10000.0);
        rate * fee_multiplier
    }
    
    /// Calculate exchange rate from token B to token A
    pub fn calculate_rate_b_to_a(&self) -> f64 {
        if self.reserve_b == 0 {
            return 0.0;
        }
        
        let rate = self.reserve_a as f64 / self.reserve_b as f64;
        let fee_multiplier = 1.0 - (self.fee_bps as f64 / 10000.0);
        rate * fee_multiplier
    }
    
    /// Get price impact for a given input amount
    pub fn calculate_price_impact(&self, amount_in: u64, input_is_token_a: bool) -> f64 {
        let (reserve_in, reserve_out) = if input_is_token_a {
            (self.reserve_a, self.reserve_b)
        } else {
            (self.reserve_b, self.reserve_a)
        };
        
        if reserve_in == 0 || reserve_out == 0 {
            return 1.0; // 100% slippage
        }
        
        // Constant product formula: k = x * y
        let k = reserve_in as f64 * reserve_out as f64;
        let new_reserve_in = reserve_in as f64 + amount_in as f64;
        let new_reserve_out = k / new_reserve_in;
        
        let amount_out = reserve_out as f64 - new_reserve_out;
        let expected_amount_out = amount_in as f64 * (reserve_out as f64 / reserve_in as f64);
        
        if expected_amount_out == 0.0 {
            return 1.0;
        }
        
        (expected_amount_out - amount_out) / expected_amount_out
    }
}

/// Main pool data fetcher with caching and batch operations
pub struct PoolDataFetcher {
    rpc_clients: Vec<Arc<RpcClient>>,
    cache: Arc<RwLock<HashMap<Pubkey, CachedPoolData>>>,
    cache_ttl_ms: u64,
    current_rpc_index: Arc<RwLock<usize>>,
}

impl PoolDataFetcher {
    /// Create a new pool data fetcher
    pub fn new(rpc_clients: Vec<Arc<RpcClient>>, cache_ttl_ms: u64) -> Self {
        info!("Initializing PoolDataFetcher with {} RPC clients, TTL: {}ms", 
            rpc_clients.len(), cache_ttl_ms);
        
        Self {
            rpc_clients,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl_ms,
            current_rpc_index: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Get next RPC client for round-robin load balancing
    async fn get_rpc_client(&self) -> Arc<RpcClient> {
        let mut index = self.current_rpc_index.write().await;
        let client = self.rpc_clients[*index % self.rpc_clients.len()].clone();
        *index = (*index + 1) % self.rpc_clients.len();
        client
    }
    
    /// Fetch multiple pools in batches with caching
    pub async fn fetch_pools_batch(&self, pool_addresses: &[Pubkey]) -> Result<Vec<PoolData>> {
        if pool_addresses.is_empty() {
            return Ok(Vec::new());
        }
        
        debug!("Fetching batch of {} pools", pool_addresses.len());
        
        // Step 1: Check cache and separate into cached/uncached
        let mut cached_pools = Vec::new();
        let mut uncached_addresses = Vec::new();
        
        {
            let cache = self.cache.read().await;
            for addr in pool_addresses {
                if let Some(cached) = cache.get(addr) {
                    if cached.is_valid(self.cache_ttl_ms) {
                        cached_pools.push(cached.data.clone());
                        continue;
                    }
                }
                uncached_addresses.push(*addr);
            }
        }
        
        debug!("Cache hit: {}, Cache miss: {}", cached_pools.len(), uncached_addresses.len());
        
        // Step 2: Fetch uncached pools in batches
        let mut fetched_pools = Vec::new();
        
        if !uncached_addresses.is_empty() {
            for chunk in uncached_addresses.chunks(MAX_BATCH_SIZE) {
                match self.fetch_accounts_with_retry(chunk).await {
                    Ok(accounts) => {
                        // Parse each account
                        for (i, account_opt) in accounts.iter().enumerate() {
                            if let Some(account) = account_opt {
                                match self.parse_pool_account(&chunk[i], account).await {
                                    Ok(pool_data) => {
                                        // Update cache
                                        self.update_cache(&chunk[i], pool_data.clone()).await;
                                        fetched_pools.push(pool_data);
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse pool {}: {}", chunk[i], e);
                                    }
                                }
                            } else {
                                warn!("Pool account {} not found", chunk[i]);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to fetch account batch: {}", e);
                    }
                }
            }
        }
        
        // Step 3: Combine cached and fetched results
        cached_pools.extend(fetched_pools);
        
        info!("Fetched {} total pools ({} cached, {} fresh)", 
            cached_pools.len(), pool_addresses.len() - uncached_addresses.len(), 
            cached_pools.len() - (pool_addresses.len() - uncached_addresses.len()));
        
        Ok(cached_pools)
    }
    
    /// Fetch accounts with retry logic and RPC failover
    async fn fetch_accounts_with_retry(&self, addresses: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        const MAX_RETRIES: usize = 3;
        let mut last_error = None;
        
        for attempt in 0..MAX_RETRIES {
            let client = self.get_rpc_client().await;
            
            match client.get_multiple_accounts_with_config(
                addresses,
                RpcAccountInfoConfig {
                    encoding: None,
                    commitment: Some(CommitmentConfig::confirmed()),
                    data_slice: None,
                    min_context_slot: None,
                },
            ).await {
                Ok(response) => {
                    return Ok(response.value);
                }
                Err(e) => {
                    warn!("RPC call failed (attempt {}/{}): {}", attempt + 1, MAX_RETRIES, e);
                    last_error = Some(e);
                    
                    // Exponential backoff
                    if attempt < MAX_RETRIES - 1 {
                        let delay = 2u64.pow(attempt as u32) * 100; // 100ms, 200ms, 400ms
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    }
                }
            }
        }
        
        Err(anyhow!("Failed after {} retries: {:?}", MAX_RETRIES, last_error))
    }
    
    /// Update cache with new pool data
    async fn update_cache(&self, address: &Pubkey, pool_data: PoolData) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        
        let mut cache = self.cache.write().await;
        cache.insert(*address, CachedPoolData {
            data: pool_data,
            timestamp,
        });
    }
    
    /// Parse pool account based on program ID
    async fn parse_pool_account(&self, address: &Pubkey, account: &Account) -> Result<PoolData> {
        let owner = account.owner;
        
        // Determine DEX type by program ID
        if owner.to_string() == RAYDIUM_AMM_V4 {
            self.parse_raydium_pool(address, account)
        } else if owner.to_string() == METEORA_DAMM {
            self.parse_meteora_damm_pool(address, account)
        } else if owner.to_string() == METEORA_VAULT {
            self.parse_meteora_vault_pool(address, account)
        } else if owner.to_string() == ORCA_WHIRLPOOL {
            self.parse_whirlpool(address, account)
        } else if owner.to_string() == ORCA_V1 {
            self.parse_orca_pool(address, account)
        } else {
            Err(anyhow!("Unknown pool program: {}", owner))
        }
    }
    
    /// Parse Raydium AMM v4 pool account
    fn parse_raydium_pool(&self, address: &Pubkey, account: &Account) -> Result<PoolData> {
        // Raydium AMM v4 account structure (simplified)
        // Offset reference: https://github.com/raydium-io/raydium-sdk
        
        if account.data.len() < 752 {
            return Err(anyhow!("Invalid Raydium pool account size"));
        }
        
        // Parse key fields (byte offsets from Raydium SDK)
        let token_a = Pubkey::try_from(&account.data[400..432])
            .map_err(|e| anyhow!("Failed to parse token_a: {}", e))?;
        let token_b = Pubkey::try_from(&account.data[432..464])
            .map_err(|e| anyhow!("Failed to parse token_b: {}", e))?;
        
        // Reserve amounts (u64 at offsets)
        let reserve_a = u64::from_le_bytes(
            account.data[504..512].try_into()
                .map_err(|e| anyhow!("Failed to parse reserve_a: {:?}", e))?
        );
        let reserve_b = u64::from_le_bytes(
            account.data[512..520].try_into()
                .map_err(|e| anyhow!("Failed to parse reserve_b: {:?}", e))?
        );
        
        // Fee (typically 25 bps for Raydium)
        let fee_bps = 25u16;
        
        debug!("Parsed Raydium pool: reserves=({}, {})", reserve_a, reserve_b);
        
        Ok(PoolData {
            pool_address: *address,
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            fee_bps,
            dex_type: DexType::Raydium,
            program_id: account.owner,
        })
    }
    
    /// Parse Meteora DAMM pool account
    fn parse_meteora_damm_pool(&self, address: &Pubkey, account: &Account) -> Result<PoolData> {
        // Meteora DAMM has a complex structure with dynamic AMM parameters
        // This is a simplified parser - full implementation would use anchor deserialize
        
        if account.data.len() < 500 {
            return Err(anyhow!("Invalid Meteora DAMM account size"));
        }
        
        // Discriminator check (first 8 bytes)
        let discriminator = &account.data[0..8];
        debug!("Meteora DAMM discriminator: {:?}", discriminator);
        
        // Parse token mints (approximate offsets)
        let token_a = Pubkey::try_from(&account.data[72..104])
            .map_err(|e| anyhow!("Failed to parse token_a: {}", e))?;
        let token_b = Pubkey::try_from(&account.data[104..136])
            .map_err(|e| anyhow!("Failed to parse token_b: {}", e))?;
        
        // Reserve amounts (u64)
        let reserve_a = u64::from_le_bytes(
            account.data[200..208].try_into()
                .map_err(|e| anyhow!("Failed to parse reserve_a: {:?}", e))?
        );
        let reserve_b = u64::from_le_bytes(
            account.data[208..216].try_into()
                .map_err(|e| anyhow!("Failed to parse reserve_b: {:?}", e))?
        );
        
        // Meteora typically has 30 bps fee
        let fee_bps = 30u16;
        
        debug!("Parsed Meteora DAMM pool: reserves=({}, {})", reserve_a, reserve_b);
        
        Ok(PoolData {
            pool_address: *address,
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            fee_bps,
            dex_type: DexType::Meteora,
            program_id: account.owner,
        })
    }
    
    /// Parse Meteora Vault pool account
    fn parse_meteora_vault_pool(&self, address: &Pubkey, account: &Account) -> Result<PoolData> {
        // Meteora Vault structure (simplified)
        
        if account.data.len() < 400 {
            return Err(anyhow!("Invalid Meteora Vault account size"));
        }
        
        // Parse tokens and reserves (approximate offsets)
        let token_a = Pubkey::try_from(&account.data[64..96])
            .map_err(|e| anyhow!("Failed to parse token_a: {}", e))?;
        let token_b = Pubkey::try_from(&account.data[96..128])
            .map_err(|e| anyhow!("Failed to parse token_b: {}", e))?;
        
        let reserve_a = u64::from_le_bytes(
            account.data[150..158].try_into()
                .map_err(|e| anyhow!("Failed to parse reserve_a: {:?}", e))?
        );
        let reserve_b = u64::from_le_bytes(
            account.data[158..166].try_into()
                .map_err(|e| anyhow!("Failed to parse reserve_b: {:?}", e))?
        );
        
        let fee_bps = 25u16;
        
        debug!("Parsed Meteora Vault pool: reserves=({}, {})", reserve_a, reserve_b);
        
        Ok(PoolData {
            pool_address: *address,
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            fee_bps,
            dex_type: DexType::Meteora,
            program_id: account.owner,
        })
    }
    
    /// Parse Orca Whirlpool (v2) account
    fn parse_whirlpool(&self, address: &Pubkey, account: &Account) -> Result<PoolData> {
        // Whirlpool uses concentrated liquidity (like Uniswap v3)
        // This is a simplified parser for current liquidity
        
        if account.data.len() < 653 {
            return Err(anyhow!("Invalid Whirlpool account size"));
        }
        
        // Parse token mints
        let token_a = Pubkey::try_from(&account.data[101..133])
            .map_err(|e| anyhow!("Failed to parse token_a: {}", e))?;
        let token_b = Pubkey::try_from(&account.data[181..213])
            .map_err(|e| anyhow!("Failed to parse token_b: {}", e))?;
        
        // Current liquidity (u128 converted to u64 for simplicity)
        let liquidity_bytes = &account.data[245..261];
        let reserve_a = u64::from_le_bytes(liquidity_bytes[0..8].try_into().unwrap());
        let reserve_b = u64::from_le_bytes(liquidity_bytes[8..16].try_into().unwrap());
        
        // Whirlpool fee (u16 at offset)
        let fee_rate = u16::from_le_bytes(
            account.data[87..89].try_into()
                .map_err(|e| anyhow!("Failed to parse fee: {:?}", e))?
        );
        let fee_bps = fee_rate / 100; // Convert from fee rate to bps
        
        debug!("Parsed Whirlpool: reserves=({}, {}), fee={} bps", reserve_a, reserve_b, fee_bps);
        
        Ok(PoolData {
            pool_address: *address,
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            fee_bps,
            dex_type: DexType::Whirlpool,
            program_id: account.owner,
        })
    }
    
    /// Parse Orca v1 (legacy) pool account
    fn parse_orca_pool(&self, address: &Pubkey, account: &Account) -> Result<PoolData> {
        // Orca v1 simple pool structure
        
        if account.data.len() < 324 {
            return Err(anyhow!("Invalid Orca pool account size"));
        }
        
        // Parse token mints (offsets from Orca SDK)
        let token_a = Pubkey::try_from(&account.data[35..67])
            .map_err(|e| anyhow!("Failed to parse token_a: {}", e))?;
        let token_b = Pubkey::try_from(&account.data[67..99])
            .map_err(|e| anyhow!("Failed to parse token_b: {}", e))?;
        
        // Reserve amounts
        let reserve_a = u64::from_le_bytes(
            account.data[220..228].try_into()
                .map_err(|e| anyhow!("Failed to parse reserve_a: {:?}", e))?
        );
        let reserve_b = u64::from_le_bytes(
            account.data[228..236].try_into()
                .map_err(|e| anyhow!("Failed to parse reserve_b: {:?}", e))?
        );
        
        // Orca v1 typically 30 bps
        let fee_bps = 30u16;
        
        debug!("Parsed Orca v1 pool: reserves=({}, {})", reserve_a, reserve_b);
        
        Ok(PoolData {
            pool_address: *address,
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            fee_bps,
            dex_type: DexType::Orca,
            program_id: account.owner,
        })
    }
    
    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        let total = cache.len();
        let valid = cache.values()
            .filter(|cached| cached.is_valid(self.cache_ttl_ms))
            .count();
        (total, valid)
    }
    
    /// Clear expired cache entries
    pub async fn clear_expired_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.retain(|_, cached| cached.is_valid(self.cache_ttl_ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_pubkey(seed: u8) -> Pubkey {
        Pubkey::new_from_array([seed; 32])
    }
    
    #[test]
    fn test_cached_pool_data_validity() {
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
        
        let cached = CachedPoolData {
            data: pool_data,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        };
        
        // Should be valid immediately
        assert!(cached.is_valid(60000)); // 60 second TTL
        
        // Create expired entry
        let expired = CachedPoolData {
            data: cached.data.clone(),
            timestamp: cached.timestamp - 120000, // 2 minutes ago
        };
        
        // Should be expired
        assert!(!expired.is_valid(60000));
    }
    
    #[test]
    fn test_pool_data_rate_calculation() {
        let pool_data = PoolData {
            pool_address: create_test_pubkey(1),
            token_a: create_test_pubkey(2),
            token_b: create_test_pubkey(3),
            reserve_a: 1000000,
            reserve_b: 2000000,
            fee_bps: 25, // 0.25%
            dex_type: DexType::Raydium,
            program_id: create_test_pubkey(100),
        };
        
        // Rate A to B: (2000000 / 1000000) * (1 - 0.0025) = 1.995
        let rate_a_to_b = pool_data.calculate_rate_a_to_b();
        assert!((rate_a_to_b - 1.995).abs() < 0.001);
        
        // Rate B to A: (1000000 / 2000000) * (1 - 0.0025) = 0.49875
        let rate_b_to_a = pool_data.calculate_rate_b_to_a();
        assert!((rate_b_to_a - 0.49875).abs() < 0.001);
    }
    
    #[test]
    fn test_price_impact_calculation() {
        let pool_data = PoolData {
            pool_address: create_test_pubkey(1),
            token_a: create_test_pubkey(2),
            token_b: create_test_pubkey(3),
            reserve_a: 1000000,
            reserve_b: 1000000,
            fee_bps: 30,
            dex_type: DexType::Raydium,
            program_id: create_test_pubkey(100),
        };
        
        // Small trade should have low impact
        let impact_small = pool_data.calculate_price_impact(1000, true);
        assert!(impact_small < 0.01); // Less than 1%
        
        // Large trade should have higher impact
        let impact_large = pool_data.calculate_price_impact(100000, true);
        assert!(impact_large > 0.05); // More than 5%
    }
}
