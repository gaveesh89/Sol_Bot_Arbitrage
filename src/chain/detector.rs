// Main Arbitrage Detection Orchestrator
//
// This module coordinates the entire detection pipeline:
// 1. Runs Bellman-Ford detection on graph updates
// 2. Focuses on high-liquidity base tokens (SOL, USDC, USDT)
// 3. Calculates profitability with slippage for realistic position sizes
// 4. Filters opportunities by minimum profit threshold
// 5. Sends profitable opportunities to execution engine
// 6. Tracks detection metrics (latency, opportunities found, etc.)

use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;
use std::time::{Duration, Instant};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use anyhow::Result;
use tracing::{info, debug, warn, error};
use chrono::Utc;

use crate::dex::triangular_arb::{SharedArbitrageGraph, BellmanFordDetector, ArbitrageCycle};

/// Main arbitrage detection orchestrator
pub struct ArbitrageDetector {
    graph: SharedArbitrageGraph,
    bellman_ford: BellmanFordDetector,
    base_tokens: Vec<Pubkey>,
    min_profit_bps: i64,
    opportunity_tx: mpsc::UnboundedSender<ArbitrageOpportunity>,
    metrics: Arc<RwLock<DetectionMetrics>>,
    max_path_length: usize,
}

/// Arbitrage opportunity ready for execution
#[derive(Clone, Debug)]
pub struct ArbitrageOpportunity {
    pub cycle: ArbitrageCycle,
    pub optimal_input_amount: u64,
    pub expected_output_amount: u64,
    pub expected_profit_sol: f64,
    pub expected_profit_bps: i64,
    pub detected_at: i64,
    pub priority_score: f64,
    pub risk_level: RiskLevel,
}

/// Risk assessment for opportunity
#[derive(Clone, Debug, PartialEq)]
pub enum RiskLevel {
    Low,      // High liquidity, 2-3 hops, reliable DEXs
    Medium,   // Moderate liquidity, 3-4 hops
    High,     // Low liquidity, 4+ hops, or untested DEXs
}

/// Detection performance metrics
#[derive(Clone, Debug, Default)]
pub struct DetectionMetrics {
    pub total_detections: u64,
    pub opportunities_found: u64,
    pub opportunities_sent: u64,
    pub avg_detection_latency_ms: f64,
    pub last_detection_time: Option<Instant>,
    pub profitable_by_token: std::collections::HashMap<String, u64>,
}

impl ArbitrageDetector {
    /// Create new arbitrage detector
    pub fn new(
        graph: SharedArbitrageGraph,
        min_profit_bps: i64,
        opportunity_tx: mpsc::UnboundedSender<ArbitrageOpportunity>,
    ) -> Self {
        // Initialize with high-liquidity base tokens
        let base_tokens = vec![
            // SOL (native token)
            Pubkey::from_str("So11111111111111111111111111111111111111112")
                .expect("Invalid SOL pubkey"),
            // USDC (most liquid stablecoin)
            Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
                .expect("Invalid USDC pubkey"),
            // USDT (second stablecoin)
            Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB")
                .expect("Invalid USDT pubkey"),
        ];

        let bellman_ford = BellmanFordDetector::new(graph.clone(), min_profit_bps);

        info!(
            "Initialized ArbitrageDetector with {} base tokens, min_profit={}bps",
            base_tokens.len(),
            min_profit_bps
        );

        Self {
            graph,
            bellman_ford,
            base_tokens,
            min_profit_bps,
            opportunity_tx,
            metrics: Arc::new(RwLock::new(DetectionMetrics::default())),
            max_path_length: 4, // 2-4 hops for triangular arbitrage
        }
    }

    /// Main detection loop - runs continuously
    pub async fn run_detection_loop(&self, mut update_signal: mpsc::UnboundedReceiver<()>) {
        info!("Starting arbitrage detection loop");
        
        loop {
            // Wait for signal that graph was updated
            match update_signal.recv().await {
                Some(()) => {
                    let start = Instant::now();
                    
                    // Run detection
                    match self.detect_all_opportunities().await {
                        Ok(count) => {
                            let latency = start.elapsed().as_millis() as f64;
                            self.update_metrics(latency, count).await;
                            
                            if count > 0 {
                                info!(
                                    "Detection completed: found {} opportunities in {:.2}ms",
                                    count, latency
                                );
                            } else {
                                debug!("Detection completed: no opportunities in {:.2}ms", latency);
                            }
                        }
                        Err(e) => {
                            error!("Detection error: {}", e);
                        }
                    }
                }
                None => {
                    warn!("Update signal channel closed, stopping detection loop");
                    break;
                }
            }
        }
    }

    /// Detect all arbitrage opportunities from all base tokens
    async fn detect_all_opportunities(&self) -> Result<usize> {
        let mut total_opportunities = 0;

        // For each base token, run detection
        for base_token in &self.base_tokens {
            let cycles = self.detect_from_token(*base_token).await?;
            
            debug!(
                "Found {} cycles starting from token {}",
                cycles.len(),
                base_token
            );

            // Process each cycle
            for cycle in cycles {
                if let Some(opportunity) = self.process_cycle(cycle).await {
                    // Send to execution engine
                    if let Err(e) = self.opportunity_tx.send(opportunity.clone()) {
                        error!("Failed to send opportunity: {}", e);
                    } else {
                        total_opportunities += 1;
                        info!(
                            "🎯 Opportunity: {:.2}% profit ({:.4} SOL), priority={:.2}, risk={:?}",
                            opportunity.expected_profit_bps as f64 / 100.0,
                            opportunity.expected_profit_sol,
                            opportunity.priority_score,
                            opportunity.risk_level
                        );
                    }
                }
            }
        }

        Ok(total_opportunities)
    }

    /// Detect arbitrage opportunities starting from a specific token
    async fn detect_from_token(&self, start_token: Pubkey) -> Result<Vec<ArbitrageCycle>> {
        // Run Bellman-Ford detection (no max_path_length parameter needed)
        let cycles = self.bellman_ford
            .detect_arbitrage(start_token)
            .await?;

        // Filter by profitability threshold
        let profitable: Vec<_> = cycles
            .into_iter()
            .filter(|cycle| cycle.gross_profit_bps >= self.min_profit_bps)
            .collect();

        Ok(profitable)
    }

    /// Process a detected cycle into an executable opportunity
    async fn process_cycle(&self, cycle: ArbitrageCycle) -> Option<ArbitrageOpportunity> {
        // Calculate optimal input amount with slippage
        let (optimal_input, expected_output, profit_sol) = 
            self.calculate_optimal_input(&cycle).await?;

        // Verify profitability after slippage
        let profit_bps = ((expected_output as i128 - optimal_input as i128) * 10000 
            / optimal_input as i128) as i64;

        if profit_bps < self.min_profit_bps {
            debug!(
                "Cycle filtered: profit {}bps < threshold {}bps after slippage",
                profit_bps, self.min_profit_bps
            );
            return None;
        }

        // Create opportunity
        let mut opportunity = ArbitrageOpportunity {
            cycle,
            optimal_input_amount: optimal_input,
            expected_output_amount: expected_output,
            expected_profit_sol: profit_sol,
            expected_profit_bps: profit_bps,
            detected_at: Utc::now().timestamp(),
            priority_score: 0.0,
            risk_level: RiskLevel::Medium,
        };

        // Calculate priority score and risk
        opportunity.priority_score = self.calculate_priority_score(&opportunity);
        opportunity.risk_level = self.assess_risk(&opportunity);

        Some(opportunity)
    }

    /// Calculate optimal input amount considering slippage
    async fn calculate_optimal_input(&self, cycle: &ArbitrageCycle) -> Option<(u64, u64, f64)> {
        // For now, use a conservative fixed amount
        // In production, implement binary search or calculus-based optimization
        // using cycle.calculate_slippage_adjusted_profit()
        
        // Use a moderate trade size: 0.1 SOL = 100M lamports
        let optimal_input = 100_000_000u64; // 0.1 SOL

        // Calculate expected output using cycle's net profit
        let profit_multiplier = 1.0 + (cycle.net_profit_after_fees / 100.0);
        let theoretical_output = (optimal_input as f64 * profit_multiplier).round() as u64;
        
        // Apply conservative slippage estimate (2%)
        let slippage_factor = 0.98;
        let expected_output = (theoretical_output as f64 * slippage_factor) as u64;

        // Calculate profit in SOL
        let profit_lamports = expected_output.saturating_sub(optimal_input) as i64;
        let profit_sol = profit_lamports as f64 / 1e9;

        if profit_lamports <= 0 {
            return None;
        }

        debug!(
            "Optimal input: {} lamports ({:.4} SOL), expected output: {} lamports, profit: {:.4} SOL",
            optimal_input,
            optimal_input as f64 / 1e9,
            expected_output,
            profit_sol
        );

        Some((optimal_input, expected_output, profit_sol))
    }

    /// Estimate slippage factor based on liquidity and trade size
    fn estimate_slippage_factor(&self, liquidity_usd: f64, trade_size_usd: f64) -> f64 {
        // Simple slippage model: slippage increases with trade size / liquidity ratio
        let ratio = trade_size_usd / liquidity_usd;
        
        if ratio < 0.01 {
            0.999 // 0.1% slippage
        } else if ratio < 0.02 {
            0.997 // 0.3% slippage
        } else if ratio < 0.05 {
            0.99  // 1% slippage
        } else {
            0.98  // 2% slippage
        }
    }

    /// Calculate priority score for opportunity (0.0 to 1.0)
    fn calculate_priority_score(&self, opp: &ArbitrageOpportunity) -> f64 {
        let mut score = 0.0;
        
        // Factor 1: Expected profit (40% weight)
        // Normalize to 0-1 scale (0 SOL = 0, 1 SOL = 1.0)
        let profit_score = (opp.expected_profit_sol / 1.0).min(1.0);
        score += profit_score * 0.4;

        // Factor 2: Number of hops (30% weight)
        // Fewer hops = better (2 hops = 1.0, 4 hops = 0.5)
        let hop_count = opp.cycle.path.len();
        let hop_score = match hop_count {
            2 => 1.0,
            3 => 0.8,
            4 => 0.6,
            _ => 0.4,
        };
        score += hop_score * 0.3;

        // Factor 3: Liquidity depth (20% weight)
        // Higher liquidity = better execution probability
        // Note: Would need liquidity data from graph, for now use default
        let liquidity_score = 0.5; // Default medium liquidity
        score += liquidity_score * 0.2;

        // Factor 4: DEX reliability (10% weight)
        // Raydium, Orca = high reliability
        let dex_score = self.calculate_dex_reliability_score(&opp.cycle);
        score += dex_score * 0.1;

        score.clamp(0.0, 1.0)
    }

    /// Calculate DEX reliability score based on historical success rates
    fn calculate_dex_reliability_score(&self, cycle: &ArbitrageCycle) -> f64 {
        use crate::dex::triangular_arb::DexType;
        let mut total_score = 0.0;
        
        for step in &cycle.path {
            let dex_score = match step.dex {
                DexType::Raydium => 1.0,      // Most reliable
                DexType::Orca => 0.95,        // Very reliable
                DexType::Whirlpool => 0.95,   // Orca Whirlpool, very reliable
                DexType::Meteora => 0.9,      // Reliable
                DexType::Pump => 0.7,         // Less tested
            };
            total_score += dex_score;
        }

        total_score / cycle.path.len() as f64
    }

    /// Assess risk level for opportunity
    fn assess_risk(&self, opp: &ArbitrageOpportunity) -> RiskLevel {
        let hop_count = opp.cycle.path.len();
        let profit_bps = opp.expected_profit_bps;

        // Low risk: 2-3 hops, high profit (>2%)
        if hop_count <= 3 && profit_bps > 200 {
            return RiskLevel::Low;
        }

        // High risk: 4+ hops or low profit (<0.5%)
        if hop_count >= 4 || profit_bps < 50 {
            return RiskLevel::High;
        }

        RiskLevel::Medium
    }

    /// Update detection metrics
    async fn update_metrics(&self, latency_ms: f64, opportunities_sent: usize) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_detections += 1;
        metrics.opportunities_found += opportunities_sent as u64;
        metrics.opportunities_sent += opportunities_sent as u64;
        
        // Update rolling average latency
        let n = metrics.total_detections as f64;
        metrics.avg_detection_latency_ms = 
            (metrics.avg_detection_latency_ms * (n - 1.0) + latency_ms) / n;
        
        metrics.last_detection_time = Some(Instant::now());
    }

    /// Get current detection metrics
    pub async fn get_metrics(&self) -> DetectionMetrics {
        self.metrics.read().await.clone()
    }

    /// Add a base token to detection list
    pub fn add_base_token(&mut self, token: Pubkey) {
        if !self.base_tokens.contains(&token) {
            self.base_tokens.push(token);
            info!("Added base token: {}", token);
        }
    }

    /// Remove a base token from detection list
    pub fn remove_base_token(&mut self, token: &Pubkey) {
        self.base_tokens.retain(|t| t != token);
        info!("Removed base token: {}", token);
    }

    /// Update minimum profit threshold
    pub fn set_min_profit(&mut self, min_profit_bps: i64) {
        self.min_profit_bps = min_profit_bps;
        info!("Updated min_profit to {}bps", min_profit_bps);
    }

    /// Update maximum path length
    pub fn set_max_path_length(&mut self, max_length: usize) {
        self.max_path_length = max_length;
        info!("Updated max_path_length to {}", max_length);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dex::triangular_arb::{create_shared_graph, CycleStep, DexType};

    #[tokio::test]
    async fn test_detector_initialization() {
        let graph = create_shared_graph();
        let (tx, _rx) = mpsc::unbounded_channel();
        
        let detector = ArbitrageDetector::new(graph, 100, tx);
        
        assert_eq!(detector.base_tokens.len(), 3);
        assert_eq!(detector.min_profit_bps, 100);
        assert_eq!(detector.max_path_length, 4);
    }

    #[tokio::test]
    async fn test_priority_score_calculation() {
        let graph = create_shared_graph();
        let (tx, _rx) = mpsc::unbounded_channel();
        let detector = ArbitrageDetector::new(graph, 100, tx);

        let sol = Pubkey::new_unique();
        let usdc = Pubkey::new_unique();

        // Create mock opportunity with CycleStep
        let cycle = ArbitrageCycle {
            path: vec![
                CycleStep {
                    from_token: sol,
                    to_token: usdc,
                    dex: DexType::Raydium,
                    pool: Pubkey::new_unique(),
                    rate: 150.0,
                    fee_bps: 25,
                },
                CycleStep {
                    from_token: usdc,
                    to_token: sol,
                    dex: DexType::Orca,
                    pool: Pubkey::new_unique(),
                    rate: 0.0068,
                    fee_bps: 30,
                },
            ],
            gross_profit_bps: 302,
            net_profit_after_fees: 0.0302,
            execution_time_estimate_ms: 1000,
            total_fee_bps: 55,
            start_token: sol,
            cycle_weight: -0.0302,
        };

        let opportunity = ArbitrageOpportunity {
            cycle,
            optimal_input_amount: 1_000_000_000,
            expected_output_amount: 1_030_200_000,
            expected_profit_sol: 0.0302,
            expected_profit_bps: 302,
            detected_at: Utc::now().timestamp(),
            priority_score: 0.0,
            risk_level: RiskLevel::Medium,
        };

        let score = detector.calculate_priority_score(&opportunity);
        
        // Should be positive score
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[tokio::test]
    async fn test_risk_assessment() {
        let graph = create_shared_graph();
        let (tx, _rx) = mpsc::unbounded_channel();
        let detector = ArbitrageDetector::new(graph, 100, tx);

        let sol = Pubkey::new_unique();
        let usdc = Pubkey::new_unique();

        // High profit, 2 hops = Low risk
        let low_risk_opp = ArbitrageOpportunity {
            cycle: ArbitrageCycle {
                path: vec![
                    CycleStep {
                        from_token: sol,
                        to_token: usdc,
                        dex: DexType::Raydium,
                        pool: Pubkey::new_unique(),
                        rate: 150.0,
                        fee_bps: 25,
                    },
                    CycleStep {
                        from_token: usdc,
                        to_token: sol,
                        dex: DexType::Orca,
                        pool: Pubkey::new_unique(),
                        rate: 0.0068,
                        fee_bps: 30,
                    },
                ],
                gross_profit_bps: 302,
                net_profit_after_fees: 0.0302,
                execution_time_estimate_ms: 1000,
                total_fee_bps: 55,
                start_token: sol,
                cycle_weight: -0.0302,
            },
            optimal_input_amount: 1_000_000_000,
            expected_output_amount: 1_030_200_000,
            expected_profit_sol: 0.0302,
            expected_profit_bps: 302,
            detected_at: Utc::now().timestamp(),
            priority_score: 0.0,
            risk_level: RiskLevel::Medium,
        };

        let risk = detector.assess_risk(&low_risk_opp);
        assert_eq!(risk, RiskLevel::Low);
    }

    #[tokio::test]
    async fn test_slippage_estimation() {
        let graph = create_shared_graph();
        let (tx, _rx) = mpsc::unbounded_channel();
        let detector = ArbitrageDetector::new(graph, 100, tx);

        // Small trade in high liquidity = low slippage
        let slippage1 = detector.estimate_slippage_factor(100_000.0, 500.0);
        assert!(slippage1 > 0.995);

        // Large trade in low liquidity = high slippage
        let slippage2 = detector.estimate_slippage_factor(10_000.0, 1_000.0);
        assert!(slippage2 < 0.995);
    }
}
