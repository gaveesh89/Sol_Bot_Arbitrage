use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time;
use tracing::{debug, error, info};

use super::token_fetch::{DexType, PoolData, TokenFetcher};

/// Token price from external source (CEX API or on-chain oracle)
#[derive(Debug, Clone)]
pub struct TokenPrice {
    pub mint: Pubkey,
    pub price_usd: f64,
    pub price_sol: f64,
    pub source: PriceSource,
    pub timestamp: SystemTime,
}

/// Price source for token pricing
#[derive(Debug, Clone, PartialEq)]
pub enum PriceSource {
    Oracle,
    CexApi,
    OnChainPool,
    Synthetic,
}

/// Price information for a token pair
#[derive(Debug, Clone)]
pub struct PriceInfo {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub price: f64, // Price of token_a in terms of token_b
    pub liquidity: u64,
    pub dex_type: DexType,
    pub pool_address: Pubkey,
    pub timestamp: std::time::SystemTime,
}

/// Arbitrage opportunity with detailed profitability analysis
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub buy_dex: DexType,
    pub buy_pool: Pubkey,
    pub buy_price: f64,
    pub sell_dex: DexType,
    pub sell_pool: Pubkey,
    pub sell_price: f64,
    pub gross_profit_bps: u64, // Gross profit in basis points
    pub net_profit_bps: i64,   // Net profit after fees/slippage
    pub estimated_slippage_bps: u64,
    pub total_fees_bps: u64,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub recommended_amount: u64,
    pub execution_risk: RiskLevel,
}

/// Risk level for arbitrage execution
#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Low,    // High liquidity, low slippage
    Medium, // Moderate liquidity
    High,   // Low liquidity, high slippage
}

/// Market data fetcher for price monitoring and arbitrage detection
pub struct MarketDataFetcher {
    token_fetcher: Arc<TokenFetcher>,
    rpc_client: Arc<RpcClient>,
    min_profit_bps: u64,
    max_slippage_bps: u64,
}

impl MarketDataFetcher {
    pub fn new(
        token_fetcher: Arc<TokenFetcher>,
        rpc_client: Arc<RpcClient>,
        min_profit_bps: u64,
        max_slippage_bps: u64,
    ) -> Self {
        info!(
            "MarketDataFetcher initialized with min profit: {} bps, max slippage: {} bps",
            min_profit_bps, max_slippage_bps
        );

        Self {
            token_fetcher,
            rpc_client,
            min_profit_bps,
            max_slippage_bps,
        }
    }

    /// Fetch token price from a specific pool
    pub async fn fetch_token_price(&self, pool_pubkey: &Pubkey, dex_type: DexType) -> Result<PriceInfo> {
        let pool_data = self
            .token_fetcher
            .fetch_pool_data(pool_pubkey, dex_type.clone())
            .await?;

        let price = self.calculate_price(&pool_data)?;

        Ok(PriceInfo {
            token_a_mint: pool_data.token_a_mint,
            token_b_mint: pool_data.token_b_mint,
            price,
            liquidity: pool_data.token_a_reserve.min(pool_data.token_b_reserve),
            dex_type: pool_data.dex_type,
            pool_address: *pool_pubkey,
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Fetch prices from multiple pools
    pub async fn fetch_multiple_prices(
        &self,
        pools: &[(Pubkey, DexType)],
    ) -> Vec<Result<PriceInfo>> {
        let mut results = Vec::new();

        for (pool_pubkey, dex_type) in pools {
            let result = self.fetch_token_price(pool_pubkey, dex_type.clone()).await;
            results.push(result);
        }

        results
    }

    /// Calculate arbitrage opportunities between different DEXs with parallel processing
    /// Considers transaction fees and slippage in profitability calculation
    pub async fn calculate_arbitrage_opportunities(
        &self,
        pools: &[(Pubkey, DexType)],
    ) -> Result<Vec<ArbitrageOpportunity>> {
        // Fetch all prices
        let price_results = self.fetch_multiple_prices(pools).await;

        // Group prices by token pair
        let mut prices_by_pair: HashMap<(Pubkey, Pubkey), Vec<PriceInfo>> = HashMap::new();

        for result in price_results {
            if let Ok(price_info) = result {
                let pair_key = normalize_pair(price_info.token_a_mint, price_info.token_b_mint);
                prices_by_pair
                    .entry(pair_key)
                    .or_insert_with(Vec::new)
                    .push(price_info);
            }
        }

        // Parallelize arbitrage calculation across all pool pairs
        let mut tasks = Vec::new();

        for ((token_a, token_b), prices) in prices_by_pair.into_iter() {
            if prices.len() < 2 {
                continue;
            }

            let min_profit_bps = self.min_profit_bps;
            let max_slippage_bps = self.max_slippage_bps;
            let token_fetcher = self.token_fetcher.clone();

            // Spawn parallel task for each token pair
            let task = tokio::task::spawn(async move {
                Self::calculate_pair_opportunities(
                    token_a,
                    token_b,
                    prices,
                    min_profit_bps,
                    max_slippage_bps,
                    token_fetcher,
                )
                .await
            });

            tasks.push(task);
        }

        // Collect results from all parallel tasks
        let mut all_opportunities = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(mut opps)) => all_opportunities.append(&mut opps),
                Ok(Err(e)) => error!("Error calculating opportunities for pair: {}", e),
                Err(e) => error!("Task join error: {}", e),
            }
        }

        // Sort by net profit (descending)
        all_opportunities.sort_by(|a, b| b.net_profit_bps.cmp(&a.net_profit_bps));

        info!("Found {} total arbitrage opportunities", all_opportunities.len());

        Ok(all_opportunities)
    }

    /// Calculate arbitrage opportunities for a specific token pair
    async fn calculate_pair_opportunities(
        token_a: Pubkey,
        token_b: Pubkey,
        prices: Vec<PriceInfo>,
        min_profit_bps: u64,
        max_slippage_bps: u64,
        token_fetcher: Arc<TokenFetcher>,
    ) -> Result<Vec<ArbitrageOpportunity>> {
        let mut opportunities = Vec::new();

        // Compare all pool pairs
        for i in 0..prices.len() {
            for j in (i + 1)..prices.len() {
                let (buy_info, sell_info) = if prices[i].price < prices[j].price {
                    (&prices[i], &prices[j])
                } else {
                    (&prices[j], &prices[i])
                };

                // Calculate gross profit in basis points
                let gross_profit_bps =
                    ((sell_info.price - buy_info.price) / buy_info.price * 10000.0) as u64;

                if gross_profit_bps < min_profit_bps {
                    continue;
                }

                // Fetch pool data for fee calculation
                let buy_pool_data = match token_fetcher
                    .fetch_pool_data(&buy_info.pool_address, buy_info.dex_type.clone())
                    .await
                {
                    Ok(data) => data,
                    Err(_) => continue,
                };

                let sell_pool_data = match token_fetcher
                    .fetch_pool_data(&sell_info.pool_address, sell_info.dex_type.clone())
                    .await
                {
                    Ok(data) => data,
                    Err(_) => continue,
                };

                // Calculate recommended trade amount
                let recommended_amount =
                    Self::calculate_trade_amount_static(buy_info, sell_info);

                // Calculate slippage for both trades
                let buy_slippage_bps =
                    (Self::estimate_slippage_static(&buy_pool_data, recommended_amount) * 10000.0)
                        as u64;
                let sell_slippage_bps =
                    (Self::estimate_slippage_static(&sell_pool_data, recommended_amount)
                        * 10000.0) as u64;
                let total_slippage_bps = buy_slippage_bps + sell_slippage_bps;

                // Skip if slippage exceeds maximum
                if total_slippage_bps > max_slippage_bps {
                    debug!(
                        "Skipping opportunity due to high slippage: {} bps",
                        total_slippage_bps
                    );
                    continue;
                }

                // Calculate total fees (DEX fees + transaction fees)
                let buy_fee_bps = (buy_pool_data.fee_numerator * 10000)
                    / buy_pool_data.fee_denominator;
                let sell_fee_bps = (sell_pool_data.fee_numerator * 10000)
                    / sell_pool_data.fee_denominator;
                let tx_fee_bps = 10; // Approximate 0.1% for transaction costs
                let total_fees_bps = buy_fee_bps + sell_fee_bps + tx_fee_bps;

                // Calculate net profit after fees and slippage
                let net_profit_bps =
                    gross_profit_bps as i64 - total_fees_bps as i64 - total_slippage_bps as i64;

                // Only include if net profit is positive
                if net_profit_bps <= 0 {
                    continue;
                }

                // Assess execution risk
                let execution_risk = Self::assess_risk(
                    buy_info.liquidity,
                    sell_info.liquidity,
                    total_slippage_bps,
                );

                let opportunity = ArbitrageOpportunity {
                    buy_dex: buy_info.dex_type.clone(),
                    buy_pool: buy_info.pool_address,
                    buy_price: buy_info.price,
                    sell_dex: sell_info.dex_type.clone(),
                    sell_pool: sell_info.pool_address,
                    sell_price: sell_info.price,
                    gross_profit_bps,
                    net_profit_bps,
                    estimated_slippage_bps: total_slippage_bps,
                    total_fees_bps,
                    token_a_mint: token_a,
                    token_b_mint: token_b,
                    recommended_amount,
                    execution_risk,
                };

                info!(
                    "Found arbitrage: Buy {:?} @ {:.6}, Sell {:?} @ {:.6} | Gross: {} bps, Net: {} bps, Fees: {} bps, Slippage: {} bps, Risk: {:?}",
                    opportunity.buy_dex,
                    opportunity.buy_price,
                    opportunity.sell_dex,
                    opportunity.sell_price,
                    opportunity.gross_profit_bps,
                    opportunity.net_profit_bps,
                    opportunity.total_fees_bps,
                    opportunity.estimated_slippage_bps,
                    opportunity.execution_risk
                );

                opportunities.push(opportunity);
            }
        }

        Ok(opportunities)
    }

    /// Assess execution risk based on liquidity and slippage
    fn assess_risk(buy_liquidity: u64, sell_liquidity: u64, slippage_bps: u64) -> RiskLevel {
        let min_liquidity = buy_liquidity.min(sell_liquidity);
        
        if min_liquidity > 1_000_000_000 && slippage_bps < 50 {
            RiskLevel::Low
        } else if min_liquidity > 100_000_000 && slippage_bps < 200 {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        }
    }

    /// Calculate price from pool data
    fn calculate_price(&self, pool_data: &PoolData) -> Result<f64> {
        if pool_data.token_b_reserve == 0 {
            return Err(anyhow::anyhow!("Zero reserve in pool"));
        }

        let price = pool_data.token_a_reserve as f64 / pool_data.token_b_reserve as f64;
        Ok(price)
    }

    /// Calculate recommended trade amount based on liquidity and risk parameters
    fn calculate_trade_amount_static(buy_price_info: &PriceInfo, sell_price_info: &PriceInfo) -> u64 {
        // Use a conservative approach: trade at most 1% of the smaller liquidity pool
        let min_liquidity = buy_price_info.liquidity.min(sell_price_info.liquidity);
        let max_trade = min_liquidity / 100;

        // Cap at reasonable maximum to limit risk
        max_trade.min(10_000_000_000) // 10 SOL equivalent max
    }

    /// Estimate slippage for a given trade amount (constant product AMM)
    fn estimate_slippage_static(pool_data: &PoolData, trade_amount: u64) -> f64 {
        // Simplified constant product AMM slippage calculation (x * y = k)
        // Real implementation should handle different AMM types (stable swap, concentrated liquidity, etc.)
        
        if pool_data.token_a_reserve == 0 || trade_amount == 0 {
            return 0.0;
        }

        let k = pool_data.token_a_reserve as f64 * pool_data.token_b_reserve as f64;
        let new_reserve_a = pool_data.token_a_reserve as f64 + trade_amount as f64;
        let new_reserve_b = k / new_reserve_a;
        let amount_out = pool_data.token_b_reserve as f64 - new_reserve_b;

        let expected_price = pool_data.token_a_reserve as f64 / pool_data.token_b_reserve as f64;
        let actual_price = trade_amount as f64 / amount_out;

        ((actual_price - expected_price) / expected_price).abs()
    }

    /// Estimate slippage (public method)
    pub fn estimate_slippage(&self, pool_data: &PoolData, trade_amount: u64) -> f64 {
        Self::estimate_slippage_static(pool_data, trade_amount)
    }
}

/// Normalize token pair to ensure consistent ordering
fn normalize_pair(token_a: Pubkey, token_b: Pubkey) -> (Pubkey, Pubkey) {
    if token_a.to_bytes() < token_b.to_bytes() {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    }
}

/// Price monitor that continuously checks for arbitrage opportunities
pub struct PriceMonitor {
    market_data_fetcher: Arc<MarketDataFetcher>,
    check_interval: Duration,
    price_threshold_bps: u64, // Minimum price change to trigger re-calculation
    last_prices: Arc<tokio::sync::RwLock<HashMap<Pubkey, f64>>>,
}

impl PriceMonitor {
    pub fn new(
        market_data_fetcher: Arc<MarketDataFetcher>,
        check_interval: Duration,
        price_threshold_bps: u64,
    ) -> Self {
        Self {
            market_data_fetcher,
            check_interval,
            price_threshold_bps,
            last_prices: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Start monitoring prices and detecting arbitrage opportunities
    /// Triggers calculation when price changes exceed threshold
    pub async fn start_monitoring(
        &self,
        pools: Vec<(Pubkey, DexType)>,
    ) -> Result<()> {
        info!(
            "Starting price monitoring for {} pools with interval {:?}, threshold {} bps",
            pools.len(),
            self.check_interval,
            self.price_threshold_bps
        );

        let mut interval = time::interval(self.check_interval);

        loop {
            interval.tick().await;

            // Fetch current prices
            let price_results = self
                .market_data_fetcher
                .fetch_multiple_prices(&pools)
                .await;

            // Check for significant price changes
            let mut significant_change = false;
            let mut last_prices = self.last_prices.write().await;

            for result in &price_results {
                if let Ok(price_info) = result {
                    let pool_key = price_info.pool_address;
                    
                    if let Some(&last_price) = last_prices.get(&pool_key) {
                        let change_bps = ((price_info.price - last_price).abs() / last_price
                            * 10000.0) as u64;
                        
                        if change_bps >= self.price_threshold_bps {
                            debug!(
                                "Significant price change detected on pool {}: {} bps",
                                pool_key, change_bps
                            );
                            significant_change = true;
                        }
                    } else {
                        // First time seeing this pool
                        significant_change = true;
                    }
                    
                    last_prices.insert(pool_key, price_info.price);
                }
            }

            drop(last_prices); // Release write lock

            // Only calculate arbitrage if there's a significant change or first iteration
            if !significant_change {
                debug!("No significant price changes, skipping arbitrage calculation");
                continue;
            }

            // Calculate arbitrage opportunities
            match self
                .market_data_fetcher
                .calculate_arbitrage_opportunities(&pools)
                .await
            {
                Ok(opportunities) => {
                    if opportunities.is_empty() {
                        debug!("No arbitrage opportunities found");
                    } else {
                        info!("Found {} arbitrage opportunities", opportunities.len());
                        
                        // Log top 5 opportunities
                        for opp in opportunities.iter().take(5) {
                            info!(
                                "Opportunity: {} -> {} | Buy: {:?} @ {:.6} | Sell: {:?} @ {:.6} | Gross: {} bps | Net: {} bps | Risk: {:?}",
                                opp.token_a_mint,
                                opp.token_b_mint,
                                opp.buy_dex,
                                opp.buy_price,
                                opp.sell_dex,
                                opp.sell_price,
                                opp.gross_profit_bps,
                                opp.net_profit_bps,
                                opp.execution_risk
                            );
                            
                            // TODO: Execute arbitrage transaction
                            // This would involve:
                            // 1. Building the transaction with swap instructions
                            // 2. Simulating the transaction
                            // 3. Sending to multiple RPCs for higher success rate
                            // 4. Monitoring confirmation
                            // 5. Handling failures and retries
                        }
                    }
                }
                Err(e) => {
                    error!("Error calculating arbitrage opportunities: {}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_pair() {
        let pubkey1 = Pubkey::new_unique();
        let pubkey2 = Pubkey::new_unique();

        let (a1, b1) = normalize_pair(pubkey1, pubkey2);
        let (a2, b2) = normalize_pair(pubkey2, pubkey1);

        assert_eq!(a1, a2);
        assert_eq!(b1, b2);
    }

    #[test]
    fn test_price_calculation() {
        // This would test the price calculation logic
        // Implement actual test cases based on your requirements
    }
}
