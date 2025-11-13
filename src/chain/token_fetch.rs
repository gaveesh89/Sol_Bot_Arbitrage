use anyhow::{Context, Result};
use dashmap::DashMap;
use moka::future::Cache;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{debug, error, info, warn};
use backoff::{ExponentialBackoff, Error as BackoffError};

/// Configuration for token fetching behavior
#[derive(Debug, Clone)]
pub struct TokenFetchConfig {
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub batch_size: usize,
    pub timeout_seconds: u64,
    pub enable_caching: bool,
    pub cache_ttl_seconds: u64,
}

impl Default for TokenFetchConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 100,
            batch_size: 100,
            timeout_seconds: 30,
            enable_caching: true,
            cache_ttl_seconds: 60,
        }
    }
}

/// Token account data with metadata
#[derive(Debug, Clone)]
pub struct TokenAccountData {
    pub pubkey: Pubkey,
    pub account: Account,
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
}

/// Cached pool entry with timestamp
#[derive(Debug, Clone)]
struct CachedPoolData {
    data: PoolData,
    timestamp: SystemTime,
}

/// Pool data structure for DEX pools - aggregates all pool information
#[derive(Debug, Clone)]
pub struct PoolData {
    pub pubkey: Pubkey,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_reserve: u64,
    pub token_b_reserve: u64,
    pub token_a_vault: Option<Pubkey>,
    pub token_b_vault: Option<Pubkey>,
    pub lp_mint: Option<Pubkey>,
    pub fee_numerator: u64,
    pub fee_denominator: u64,
    pub dex_type: DexType,
    pub last_updated: SystemTime,
}

impl PoolData {
    /// Check if cached pool data is still valid
    pub fn is_valid(&self, ttl_seconds: u64) -> bool {
        if let Ok(elapsed) = self.last_updated.elapsed() {
            elapsed.as_secs() < ttl_seconds
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DexType {
    Raydium,
    Meteora,
    Whirlpool,
    Orca,
    Pump,
}

/// TokenFetcher with enhanced caching, batching, and retry logic
pub struct TokenFetcher {
    rpc_client: Arc<RpcClient>,
    config: TokenFetchConfig,
    // Moka cache for account data
    account_cache: Cache<Pubkey, Account>,
    // DashMap for concurrent pool data cache with timestamp
    pool_cache: Arc<DashMap<Pubkey, CachedPoolData>>,
}

impl TokenFetcher {
    pub fn new(
        rpc_client: Arc<RpcClient>,
        cache_ttl: Duration,
        cache_max_size: u64,
        max_retries: u32,
    ) -> Self {
        let config = TokenFetchConfig {
            max_retries,
            cache_ttl_seconds: cache_ttl.as_secs(),
            ..Default::default()
        };

        let account_cache = Cache::builder()
            .max_capacity(cache_max_size)
            .time_to_live(cache_ttl)
            .build();

        let pool_cache = Arc::new(DashMap::new());

        info!(
            "TokenFetcher initialized - cache TTL: {:?}, max size: {}, batch size: {}",
            cache_ttl, cache_max_size, config.batch_size
        );

        Self {
            rpc_client,
            config,
            account_cache,
            pool_cache,
        }
    }

    pub fn with_config(
        rpc_client: Arc<RpcClient>,
        config: TokenFetchConfig,
        cache_max_size: u64,
    ) -> Self {
        let account_cache = Cache::builder()
            .max_capacity(cache_max_size)
            .time_to_live(Duration::from_secs(config.cache_ttl_seconds))
            .build();

        let pool_cache = Arc::new(DashMap::new());

        info!(
            "TokenFetcher initialized with custom config - batch size: {}, max retries: {}",
            config.batch_size, config.max_retries
        );

        Self {
            rpc_client,
            config,
            account_cache,
            pool_cache,
        }
    }

    /// Fetch account with retry logic and caching
    pub async fn fetch_account(&self, pubkey: &Pubkey) -> Result<Account> {
        // Check cache first
        if let Some(account) = self.account_cache.get(pubkey).await {
            debug!("Cache hit for account: {}", pubkey);
            return Ok(account);
        }

        debug!("Cache miss for account: {}, fetching from RPC", pubkey);

        // Fetch with retry logic
        let account = self.fetch_account_with_retry(pubkey).await?;

        // Update cache
        self.account_cache.insert(*pubkey, account.clone()).await;

        Ok(account)
    }

    /// Fetch multiple accounts in batch with retry logic
    pub async fn fetch_accounts_batch(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        let mut to_fetch = Vec::new();
        let mut results = vec![None; pubkeys.len()];

        // Check cache for each account
        for (i, pubkey) in pubkeys.iter().enumerate() {
            if let Some(account) = self.account_cache.get(pubkey).await {
                debug!("Cache hit for account: {}", pubkey);
                results[i] = Some(account);
            } else {
                to_fetch.push((i, *pubkey));
            }
        }

        if to_fetch.is_empty() {
            return Ok(results);
        }

        debug!("Fetching {} accounts in batch", to_fetch.len());

        // Fetch missing accounts
        let pubkeys_to_fetch: Vec<Pubkey> = to_fetch.iter().map(|(_, pk)| *pk).collect();
        let fetched_accounts = self
            .fetch_accounts_batch_with_retry(&pubkeys_to_fetch)
            .await?;

        // Update results and cache
        for ((i, pubkey), account) in to_fetch.into_iter().zip(fetched_accounts.into_iter()) {
            if let Some(ref acc) = account {
                self.account_cache.insert(pubkey, acc.clone()).await;
            }
            results[i] = account;
        }

        Ok(results)
    }

    /// Fetch pool data with caching
    pub async fn fetch_pool_data(&self, pool_pubkey: &Pubkey, dex_type: DexType) -> Result<PoolData> {
        // Check cache first if enabled
        if self.config.enable_caching {
            if let Some(cached) = self.pool_cache.get(pool_pubkey) {
                if cached.data.is_valid(self.config.cache_ttl_seconds) {
                    debug!("Cache hit for pool: {}", pool_pubkey);
                    return Ok(cached.data.clone());
                } else {
                    debug!("Cache expired for pool: {}", pool_pubkey);
                    self.pool_cache.remove(pool_pubkey);
                }
            }
        }

        debug!("Cache miss for pool: {}, fetching from RPC", pool_pubkey);

        // Fetch pool account
        let account = self.fetch_account(pool_pubkey).await?;

        // Parse pool data based on DEX type
        let mut pool_data = self.parse_pool_data(pool_pubkey, &account, dex_type)?;
        pool_data.last_updated = SystemTime::now();

        // Update cache if enabled
        if self.config.enable_caching {
            self.pool_cache.insert(
                *pool_pubkey,
                CachedPoolData {
                    data: pool_data.clone(),
                    timestamp: SystemTime::now(),
                },
            );
        }

        Ok(pool_data)
    }

    /// Initialize pool data for multiple pools with batching and retry logic
    /// This aggregates all DEX pool data for a given mint
    pub async fn initialize_pool_data(&self, pool_configs: &[(Pubkey, DexType)]) -> Result<Vec<PoolData>> {
        info!("Initializing pool data for {} pools", pool_configs.len());
        
        let mut all_pool_data = Vec::new();
        let mut failed_pools = Vec::new();

        // Process pools in batches
        for chunk in pool_configs.chunks(self.config.batch_size) {
            debug!("Processing batch of {} pools", chunk.len());
            
            // Collect pubkeys for batch fetch
            let pubkeys: Vec<Pubkey> = chunk.iter().map(|(pk, _)| *pk).collect();
            
            // Fetch accounts in batch with retry
            match self.fetch_accounts_batch(&pubkeys).await {
                Ok(accounts) => {
                    // Parse each account
                    for ((pubkey, dex_type), account_opt) in 
                        chunk.iter().zip(accounts.iter()) 
                    {
                        if let Some(account) = account_opt {
                            match self.parse_pool_data(pubkey, account, dex_type.clone()) {
                                Ok(mut pool_data) => {
                                    pool_data.last_updated = SystemTime::now();
                                    
                                    // Cache the pool data
                                    if self.config.enable_caching {
                                        self.pool_cache.insert(
                                            *pubkey,
                                            CachedPoolData {
                                                data: pool_data.clone(),
                                                timestamp: SystemTime::now(),
                                            },
                                        );
                                    }
                                    
                                    all_pool_data.push(pool_data);
                                    debug!("Initialized pool {} ({:?})", pubkey, dex_type);
                                }
                                Err(e) => {
                                    warn!("Failed to parse pool {} ({:?}): {}", pubkey, dex_type, e);
                                    failed_pools.push((*pubkey, dex_type.clone()));
                                }
                            }
                        } else {
                            warn!("Pool account not found: {} ({:?})", pubkey, dex_type);
                            failed_pools.push((*pubkey, dex_type.clone()));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch batch of pools: {}", e);
                    failed_pools.extend(chunk.iter().cloned());
                }
            }
        }

        info!(
            "Pool initialization complete: {} succeeded, {} failed",
            all_pool_data.len(),
            failed_pools.len()
        );

        if !failed_pools.is_empty() {
            warn!("Failed pools: {:?}", failed_pools);
        }

        Ok(all_pool_data)
    }

    /// Invalidate cache for a specific account
    pub async fn invalidate_account_cache(&self, pubkey: &Pubkey) {
        self.account_cache.invalidate(pubkey).await;
        debug!("Invalidated cache for account: {}", pubkey);
    }

    /// Invalidate cache for a specific pool
    pub fn invalidate_pool_cache(&self, pubkey: &Pubkey) {
        self.pool_cache.remove(pubkey);
        debug!("Invalidated cache for pool: {}", pubkey);
    }

    /// Clear all caches
    pub fn clear_all_caches(&self) {
        self.account_cache.invalidate_all();
        self.pool_cache.clear();
        info!("Cleared all caches");
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        CacheStats {
            account_cache_size: self.account_cache.entry_count(),
            pool_cache_size: self.pool_cache.len(),
            account_cache_hits: self.account_cache.weighted_size(), // Approximation
            pool_cache_hits: 0, // DashMap doesn't track hits
        }
    }

    // Private helper methods

    async fn fetch_account_with_retry(&self, pubkey: &Pubkey) -> Result<Account> {
        let backoff = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(30)),
            ..Default::default()
        };

        let fetch_operation = || async {
            match self.rpc_client.get_account(pubkey).await {
                Ok(account) => Ok(account),
                Err(e) => {
                    warn!("Failed to fetch account {}: {}", pubkey, e);
                    Err(BackoffError::transient(e))
                }
            }
        };

        backoff::future::retry(backoff, fetch_operation)
            .await
            .context(format!("Failed to fetch account {} after retries", pubkey))
    }

    async fn fetch_accounts_batch_with_retry(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        let backoff = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(30)),
            ..Default::default()
        };

        let fetch_operation = || async {
            match self.rpc_client.get_multiple_accounts(pubkeys).await {
                Ok(accounts) => Ok(accounts),
                Err(e) => {
                    warn!("Failed to fetch accounts batch: {}", e);
                    Err(BackoffError::transient(e))
                }
            }
        };

        backoff::future::retry(backoff, fetch_operation)
            .await
            .context("Failed to fetch accounts batch after retries")
    }

    fn parse_pool_data(&self, pool_pubkey: &Pubkey, account: &Account, dex_type: DexType) -> Result<PoolData> {
        // This is a simplified parser - you'll need to implement proper parsing for each DEX type
        // based on their account structures
        
        match dex_type {
            DexType::Raydium => self.parse_raydium_pool(pool_pubkey, account),
            DexType::Meteora => self.parse_meteora_pool(pool_pubkey, account),
            DexType::Whirlpool => self.parse_whirlpool_pool(pool_pubkey, account),
            DexType::Orca => self.parse_orca_pool(pool_pubkey, account),
            DexType::Pump => self.parse_pump_pool(pool_pubkey, account),
        }
    }

    fn parse_raydium_pool(&self, pool_pubkey: &Pubkey, _account: &Account) -> Result<PoolData> {
        // Placeholder - implement actual Raydium pool parsing
        // You'll need to deserialize the account data according to Raydium's pool structure
        warn!("Raydium pool parsing not fully implemented");
        
        Ok(PoolData {
            pubkey: *pool_pubkey,
            token_a_mint: Pubkey::default(),
            token_b_mint: Pubkey::default(),
            token_a_reserve: 0,
            token_b_reserve: 0,
            token_a_vault: None,
            token_b_vault: None,
            lp_mint: None,
            fee_numerator: 25,
            fee_denominator: 10000,
            dex_type: DexType::Raydium,
            last_updated: SystemTime::now(),
        })
    }

    fn parse_meteora_pool(&self, pool_pubkey: &Pubkey, _account: &Account) -> Result<PoolData> {
        // Placeholder - implement actual Meteora pool parsing
        warn!("Meteora pool parsing not fully implemented");
        
        Ok(PoolData {
            pubkey: *pool_pubkey,
            token_a_mint: Pubkey::default(),
            token_b_mint: Pubkey::default(),
            token_a_reserve: 0,
            token_b_reserve: 0,
            token_a_vault: None,
            token_b_vault: None,
            lp_mint: None,
            fee_numerator: 20,
            fee_denominator: 10000,
            dex_type: DexType::Meteora,
            last_updated: SystemTime::now(),
        })
    }

    fn parse_whirlpool_pool(&self, pool_pubkey: &Pubkey, _account: &Account) -> Result<PoolData> {
        // Placeholder - implement actual Whirlpool pool parsing
        warn!("Whirlpool pool parsing not fully implemented");
        
        Ok(PoolData {
            pubkey: *pool_pubkey,
            token_a_mint: Pubkey::default(),
            token_b_mint: Pubkey::default(),
            token_a_reserve: 0,
            token_b_reserve: 0,
            token_a_vault: None,
            token_b_vault: None,
            lp_mint: None,
            fee_numerator: 30,
            fee_denominator: 10000,
            dex_type: DexType::Whirlpool,
            last_updated: SystemTime::now(),
        })
    }

    fn parse_orca_pool(&self, pool_pubkey: &Pubkey, _account: &Account) -> Result<PoolData> {
        // Placeholder - implement actual Orca pool parsing
        warn!("Orca pool parsing not fully implemented");
        
        Ok(PoolData {
            pubkey: *pool_pubkey,
            token_a_mint: Pubkey::default(),
            token_b_mint: Pubkey::default(),
            token_a_reserve: 0,
            token_b_reserve: 0,
            token_a_vault: None,
            token_b_vault: None,
            lp_mint: None,
            fee_numerator: 30,
            fee_denominator: 10000,
            dex_type: DexType::Orca,
            last_updated: SystemTime::now(),
        })
    }

    fn parse_pump_pool(&self, pool_pubkey: &Pubkey, _account: &Account) -> Result<PoolData> {
        // Placeholder - implement actual Pump pool parsing
        warn!("Pump pool parsing not fully implemented");
        
        Ok(PoolData {
            pubkey: *pool_pubkey,
            token_a_mint: Pubkey::default(),
            token_b_mint: Pubkey::default(),
            token_a_reserve: 0,
            token_b_reserve: 0,
            token_a_vault: None,
            token_b_vault: None,
            lp_mint: None,
            fee_numerator: 100,
            fee_denominator: 10000,
            dex_type: DexType::Pump,
            last_updated: SystemTime::now(),
        })
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub account_cache_size: u64,
    pub pool_cache_size: usize,
    pub account_cache_hits: u64,
    pub pool_cache_hits: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_fetcher_creation() {
        let rpc_client = Arc::new(RpcClient::new("https://api.mainnet-beta.solana.com".to_string()));
        let fetcher = TokenFetcher::new(
            rpc_client,
            Duration::from_secs(60),
            10000,
            3,
        );
        
        assert_eq!(fetcher.config.max_retries, 3);
    }
}
