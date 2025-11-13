use anyhow::{Context, Result};
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Main configuration struct containing all bot settings
#[derive(Debug, Clone)]
pub struct Config {
    pub bot: BotConfig,
    pub routing: RoutingConfig,
    pub mints: Vec<MintConfig>,
    pub rpc: RpcConfig,
    pub spam: SpamConfig,
    pub wallet: WalletConfig,
    pub flashloan: FlashloanConfig,
    pub cache: CacheConfig,
    pub monitoring: MonitoringConfig,
    pub execution: ExecutionConfig,
    pub dex: DexConfig,
}

/// Bot behavior configuration
#[derive(Debug, Clone, Deserialize)]
pub struct BotConfig {
    pub min_profit_bps: u64,
    pub max_slippage_bps: u64,
    pub transaction_timeout_ms: u64,
    pub max_retries: u32,
    pub enable_arbitrage: bool,
    pub enable_sandwich: bool,
    pub max_position_size: u64,
}

/// Routing and pathfinding configuration
#[derive(Debug, Clone, Deserialize)]
pub struct RoutingConfig {
    pub max_hops: u32,
    pub enable_multi_hop: bool,
    pub prefer_direct_routes: bool,
    pub route_cache_ttl_seconds: u64,
}

/// Mint (token) configuration with pool associations
#[derive(Debug, Clone)]
pub struct MintConfig {
    pub address: Pubkey,
    pub symbol: String,
    pub decimals: u8,
    pub pools: Vec<Pubkey>,
    pub is_quote: bool,
}

/// RPC endpoint configuration
#[derive(Debug, Clone, Deserialize)]
pub struct RpcConfig {
    pub url: String,
    pub ws_url: String,
    pub backup_urls: Vec<String>,
    pub commitment_level: String,
    pub timeout_seconds: u64,
}

/// Transaction spam configuration for higher inclusion probability
#[derive(Debug, Clone, Deserialize)]
pub struct SpamConfig {
    pub enabled: bool,
    pub num_submissions: u32,
    pub delay_ms: u64,
    pub use_different_rpcs: bool,
}

/// Wallet configuration
#[derive(Debug, Clone, Deserialize)]
pub struct WalletConfig {
    pub keypair_path: Option<String>,
    pub private_key: Option<String>,
    pub min_balance_sol: f64,
}

/// Flash loan configuration
#[derive(Debug, Clone, Deserialize)]
pub struct FlashloanConfig {
    pub enabled: bool,
    pub provider: String,
    pub max_loan_amount: u64,
    pub fee_bps: u64,
}

/// Cache configuration
#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    pub ttl_seconds: u64,
    pub max_size: u64,
    pub enable_pool_cache: bool,
    pub enable_account_cache: bool,
}

/// Monitoring and metrics configuration
#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringConfig {
    pub price_check_interval_ms: u64,
    pub price_change_threshold_bps: u64, // Minimum price change to trigger arbitrage calc
    pub enable_metrics: bool,
    pub log_level: String,
    pub enable_performance_tracking: bool,
}

/// Transaction execution configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionConfig {
    pub compute_unit_limit: u32,
    pub compute_unit_price: u64,
    pub priority_fee_percentile: u8,
    pub use_versioned_transactions: bool,
    pub simulate_before_send: bool,
}

/// DEX program IDs configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DexConfig {
    pub raydium_program_id: Pubkey,
    pub raydium_amm_program_id: Pubkey,
    pub meteora_dlmm_program_id: Pubkey,
    pub meteora_pools_program_id: Pubkey,
    pub meteora_damm_program_id: Pubkey,
    pub meteora_vault_program_id: Pubkey,
    pub whirlpool_program_id: Pubkey,
    pub orca_program_id: Pubkey,
    pub pump_program_id: Pubkey,
}

impl Config {
    /// Load configuration from environment variables
    pub fn load() -> Result<Self> {
        // Load environment variables from .env file
        dotenvy::dotenv().ok();

        // Bot configuration
        let bot = BotConfig {
            min_profit_bps: get_u64_env("MIN_PROFIT_BPS", 50)?,
            max_slippage_bps: get_u64_env("MAX_SLIPPAGE_BPS", 100)?,
            transaction_timeout_ms: get_u64_env("TRANSACTION_TIMEOUT_MS", 30000)?,
            max_retries: get_u32_env("MAX_RETRIES", 3)?,
            enable_arbitrage: get_bool_env("ENABLE_ARBITRAGE", true),
            enable_sandwich: get_bool_env("ENABLE_SANDWICH", false),
            max_position_size: get_u64_env("MAX_POSITION_SIZE", 1_000_000_000)?, // 1 SOL default
        };

        // Routing configuration
        let routing = RoutingConfig {
            max_hops: get_u32_env("MAX_HOPS", 3)?,
            enable_multi_hop: get_bool_env("ENABLE_MULTI_HOP", true),
            prefer_direct_routes: get_bool_env("PREFER_DIRECT_ROUTES", true),
            route_cache_ttl_seconds: get_u64_env("ROUTE_CACHE_TTL_SECONDS", 300)?,
        };

        // Load mint configurations from environment variables
        let mints = Self::load_mint_configs()?;

        // RPC configuration
        let rpc = RpcConfig {
            url: std::env::var("RPC_URL").context("RPC_URL not set")?,
            ws_url: get_env_or_default("RPC_WS_URL", "wss://api.mainnet-beta.solana.com"),
            backup_urls: parse_string_list(&get_env_or_default("BACKUP_RPC_URLS", "")),
            commitment_level: get_env_or_default("COMMITMENT_LEVEL", "confirmed"),
            timeout_seconds: get_u64_env("RPC_TIMEOUT_SECONDS", 30)?,
        };

        // Spam configuration
        let spam = SpamConfig {
            enabled: get_bool_env("SPAM_ENABLED", true),
            num_submissions: get_u32_env("SPAM_NUM_SUBMISSIONS", 3)?,
            delay_ms: get_u64_env("SPAM_DELAY_MS", 10)?,
            use_different_rpcs: get_bool_env("SPAM_USE_DIFFERENT_RPCS", true),
        };

        // Wallet configuration
        let wallet = WalletConfig {
            keypair_path: std::env::var("WALLET_KEYPAIR_PATH").ok(),
            private_key: std::env::var("WALLET_PRIVATE_KEY").ok(),
            min_balance_sol: std::env::var("MIN_BALANCE_SOL")
                .unwrap_or_else(|_| "0.1".to_string())
                .parse()
                .unwrap_or(0.1),
        };

        // Flash loan configuration
        let flashloan = FlashloanConfig {
            enabled: get_bool_env("FLASHLOAN_ENABLED", false),
            provider: get_env_or_default("FLASHLOAN_PROVIDER", "solend"),
            max_loan_amount: get_u64_env("FLASHLOAN_MAX_AMOUNT", 100_000_000_000)?, // 100 SOL
            fee_bps: get_u64_env("FLASHLOAN_FEE_BPS", 9)?,
        };

        // Cache configuration
        let cache = CacheConfig {
            ttl_seconds: get_u64_env("CACHE_TTL_SECONDS", 60)?,
            max_size: get_u64_env("CACHE_MAX_SIZE", 10000)?,
            enable_pool_cache: get_bool_env("ENABLE_POOL_CACHE", true),
            enable_account_cache: get_bool_env("ENABLE_ACCOUNT_CACHE", true),
        };

        // Monitoring configuration
        let monitoring = MonitoringConfig {
            price_check_interval_ms: get_u64_env("PRICE_CHECK_INTERVAL_MS", 1000)?,
            price_change_threshold_bps: get_u64_env("PRICE_CHANGE_THRESHOLD_BPS", 50)?,
            enable_metrics: get_bool_env("ENABLE_METRICS", false),
            log_level: get_env_or_default("LOG_LEVEL", "info"),
            enable_performance_tracking: get_bool_env("ENABLE_PERFORMANCE_TRACKING", true),
        };

        // Execution configuration
        let execution = ExecutionConfig {
            compute_unit_limit: get_u32_env("COMPUTE_UNIT_LIMIT", 200000)?,
            compute_unit_price: get_u64_env("COMPUTE_UNIT_PRICE", 1000)?,
            priority_fee_percentile: std::env::var("PRIORITY_FEE_PERCENTILE")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .context("Failed to parse PRIORITY_FEE_PERCENTILE")?,
            use_versioned_transactions: get_bool_env("USE_VERSIONED_TRANSACTIONS", true),
            simulate_before_send: get_bool_env("SIMULATE_BEFORE_SEND", true),
        };

        // DEX configuration
        let dex = DexConfig {
            raydium_program_id: parse_pubkey("RAYDIUM_PROGRAM_ID")?,
            raydium_amm_program_id: parse_pubkey("RAYDIUM_AMM_PROGRAM_ID")?,
            meteora_dlmm_program_id: parse_pubkey("METEORA_DLMM_PROGRAM_ID")?,
            meteora_pools_program_id: parse_pubkey("METEORA_POOLS_PROGRAM_ID")?,
            meteora_damm_program_id: parse_pubkey("METEORA_DAMM_PROGRAM_ID")?,
            meteora_vault_program_id: parse_pubkey("METEORA_VAULT_PROGRAM_ID")?,
            whirlpool_program_id: parse_pubkey("WHIRLPOOL_PROGRAM_ID")?,
            orca_program_id: parse_pubkey("ORCA_PROGRAM_ID")?,
            pump_program_id: parse_pubkey("PUMP_PROGRAM_ID")?,
        };

        Ok(Config {
            bot,
            routing,
            mints,
            rpc,
            spam,
            wallet,
            flashloan,
            cache,
            monitoring,
            execution,
            dex,
        })
    }

    /// Load mint configurations from environment variables (MINT_1, MINT_2, etc.)
    fn load_mint_configs() -> Result<Vec<MintConfig>> {
        let mut mints = Vec::new();
        let mut index = 1;

        loop {
            let prefix = format!("MINT_{}", index);
            
            // Check if this mint exists
            let address_key = format!("{}_ADDRESS", prefix);
            if std::env::var(&address_key).is_err() {
                break;
            }

            let address = parse_pubkey_from_env(&address_key)?;
            let symbol = get_env_or_default(&format!("{}_SYMBOL", prefix), &format!("TOKEN{}", index));
            let decimals = get_u32_env(&format!("{}_DECIMALS", prefix), 9)? as u8;
            let is_quote = get_bool_env(&format!("{}_IS_QUOTE", prefix), false);
            
            // Parse pool addresses (comma-separated)
            let pools_str = get_env_or_default(&format!("{}_POOLS", prefix), "");
            let pools = parse_pubkey_list(&pools_str)?;

            mints.push(MintConfig {
                address,
                symbol,
                decimals,
                pools,
                is_quote,
            });

            index += 1;
        }

        // If no mints found, load default common tokens
        if mints.is_empty() {
            mints = Self::load_default_mints()?;
        }

        Ok(mints)
    }

    /// Load default mint configurations for common tokens
    fn load_default_mints() -> Result<Vec<MintConfig>> {
        let mut mints = Vec::new();

        // SOL (Wrapped SOL)
        if let Ok(sol) = parse_pubkey_optional("SOL_MINT") {
            mints.push(MintConfig {
                address: sol,
                symbol: "SOL".to_string(),
                decimals: 9,
                pools: vec![],
                is_quote: true,
            });
        }

        // USDC
        if let Ok(usdc) = parse_pubkey_optional("USDC_MINT") {
            mints.push(MintConfig {
                address: usdc,
                symbol: "USDC".to_string(),
                decimals: 6,
                pools: vec![],
                is_quote: true,
            });
        }

        // USDT
        if let Ok(usdt) = parse_pubkey_optional("USDT_MINT") {
            mints.push(MintConfig {
                address: usdt,
                symbol: "USDT".to_string(),
                decimals: 6,
                pools: vec![],
                is_quote: true,
            });
        }

        Ok(mints)
    }
}

// ============================================================================
// Helper Functions for Environment Variable Parsing
// ============================================================================

/// Get environment variable or return default value
fn get_env_or_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Get boolean environment variable with default
fn get_bool_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .unwrap_or_else(|_| default.to_string())
        .parse()
        .unwrap_or(default)
}

/// Get u32 environment variable with default
fn get_u32_env(key: &str, default: u32) -> Result<u32> {
    Ok(std::env::var(key)
        .unwrap_or_else(|_| default.to_string())
        .parse()
        .context(format!("Failed to parse {} as u32", key))?)
}

/// Get u64 environment variable with default
fn get_u64_env(key: &str, default: u64) -> Result<u64> {
    Ok(std::env::var(key)
        .unwrap_or_else(|_| default.to_string())
        .parse()
        .context(format!("Failed to parse {} as u64", key))?)
}

/// Parse comma-separated string list
fn parse_string_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse pubkey from environment variable (required)
fn parse_pubkey(env_var: &str) -> Result<Pubkey> {
    let pubkey_str = std::env::var(env_var).context(format!("{} not set", env_var))?;
    Pubkey::from_str(&pubkey_str).context(format!("Failed to parse {} as Pubkey", env_var))
}

/// Parse pubkey from environment variable name directly
fn parse_pubkey_from_env(env_var: &str) -> Result<Pubkey> {
    let pubkey_str = std::env::var(env_var).context(format!("{} not set", env_var))?;
    Pubkey::from_str(&pubkey_str).context(format!("Failed to parse {} as Pubkey", env_var))
}

/// Parse optional pubkey from environment variable
fn parse_pubkey_optional(env_var: &str) -> Result<Pubkey> {
    let pubkey_str = std::env::var(env_var)?;
    Pubkey::from_str(&pubkey_str).context(format!("Failed to parse {} as Pubkey", env_var))
}

/// Parse comma-separated list of pubkeys
fn parse_pubkey_list(input: &str) -> Result<Vec<Pubkey>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            Pubkey::from_str(s).context(format!("Failed to parse '{}' as Pubkey", s))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pubkey() {
        std::env::set_var(
            "TEST_PUBKEY",
            "So11111111111111111111111111111111111111112",
        );
        let pubkey = parse_pubkey("TEST_PUBKEY").unwrap();
        assert_eq!(
            pubkey.to_string(),
            "So11111111111111111111111111111111111111112"
        );
    }
}
