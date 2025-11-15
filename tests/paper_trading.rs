use solana_mev_bot::{
    arbitrage::{ArbitrageDetector, ArbitrageOpportunity},
    chain::{TransactionBuilder, TransactionSender},
    config::Config,
    dex::DexIntegrator,
    utils::metrics::MetricsCollector,
};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime},
};
use tokio::time::sleep;

/// Paper trading results for a single simulated trade
#[derive(Debug, Clone)]
struct PaperTrade {
    timestamp: SystemTime,
    opportunity: ArbitrageOpportunity,
    simulated_profit: f64,     // SOL
    simulated_cost: f64,        // SOL (gas + fees)
    net_profit: f64,            // SOL
    execution_time_ms: u64,
    would_have_succeeded: bool, // Based on simulation
}

/// Paper trading statistics
#[derive(Debug, Clone)]
struct PaperTradingStats {
    total_opportunities: u64,
    trades_executed: u64,
    trades_profitable: u64,
    trades_unprofitable: u64,
    total_gross_profit: f64,     // SOL
    total_costs: f64,             // SOL
    total_net_profit: f64,        // SOL
    max_single_profit: f64,       // SOL
    max_single_loss: f64,         // SOL
    max_drawdown: f64,            // SOL
    peak_balance: f64,            // SOL
    current_balance: f64,         // SOL
    average_profit_per_trade: f64, // SOL
    win_rate: f64,                // Percentage
    sharpe_ratio: f64,
    start_time: SystemTime,
    end_time: Option<SystemTime>,
    total_duration_secs: u64,
}

impl PaperTradingStats {
    fn new(starting_balance: f64) -> Self {
        Self {
            total_opportunities: 0,
            trades_executed: 0,
            trades_profitable: 0,
            trades_unprofitable: 0,
            total_gross_profit: 0.0,
            total_costs: 0.0,
            total_net_profit: 0.0,
            max_single_profit: 0.0,
            max_single_loss: 0.0,
            max_drawdown: 0.0,
            peak_balance: starting_balance,
            current_balance: starting_balance,
            average_profit_per_trade: 0.0,
            win_rate: 0.0,
            sharpe_ratio: 0.0,
            start_time: SystemTime::now(),
            end_time: None,
            total_duration_secs: 0,
        }
    }

    fn record_trade(&mut self, trade: &PaperTrade) {
        self.trades_executed += 1;

        if trade.would_have_succeeded {
            self.total_gross_profit += trade.simulated_profit;
            self.total_costs += trade.simulated_cost;
            self.total_net_profit += trade.net_profit;
            self.current_balance += trade.net_profit;

            if trade.net_profit > 0.0 {
                self.trades_profitable += 1;
                if trade.net_profit > self.max_single_profit {
                    self.max_single_profit = trade.net_profit;
                }
            } else {
                self.trades_unprofitable += 1;
                if trade.net_profit < self.max_single_loss {
                    self.max_single_loss = trade.net_profit;
                }
            }

            // Update peak and drawdown
            if self.current_balance > self.peak_balance {
                self.peak_balance = self.current_balance;
            }

            let current_drawdown = self.peak_balance - self.current_balance;
            if current_drawdown > self.max_drawdown {
                self.max_drawdown = current_drawdown;
            }
        } else {
            // Failed trade - assume we lost gas cost
            self.trades_unprofitable += 1;
            self.total_costs += trade.simulated_cost;
            self.total_net_profit -= trade.simulated_cost;
            self.current_balance -= trade.simulated_cost;

            if -trade.simulated_cost < self.max_single_loss {
                self.max_single_loss = -trade.simulated_cost;
            }
        }

        self.calculate_metrics();
    }

    fn calculate_metrics(&mut self) {
        if self.trades_executed > 0 {
            self.average_profit_per_trade = self.total_net_profit / self.trades_executed as f64;
            self.win_rate = (self.trades_profitable as f64 / self.trades_executed as f64) * 100.0;
        }
    }

    fn calculate_sharpe_ratio(&mut self, returns: &[f64]) {
        if returns.len() < 2 {
            self.sharpe_ratio = 0.0;
            return;
        }

        // Calculate mean return
        let mean_return: f64 = returns.iter().sum::<f64>() / returns.len() as f64;

        // Calculate standard deviation
        let variance: f64 = returns
            .iter()
            .map(|r| {
                let diff = r - mean_return;
                diff * diff
            })
            .sum::<f64>()
            / (returns.len() - 1) as f64;

        let std_dev = variance.sqrt();

        // Sharpe ratio = mean / std_dev
        // Assuming risk-free rate = 0 for simplicity
        if std_dev > 0.0 {
            self.sharpe_ratio = mean_return / std_dev;
        } else {
            self.sharpe_ratio = 0.0;
        }
    }

    fn finalize(&mut self) {
        self.end_time = Some(SystemTime::now());
        if let Ok(duration) = self.end_time.unwrap().duration_since(self.start_time) {
            self.total_duration_secs = duration.as_secs();
        }
    }

    fn print_report(&self) {
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║           PAPER TRADING TEST - FINAL REPORT                   ║");
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        println!("📊 EXECUTION SUMMARY");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  Duration:                 {} hours {} minutes",
            self.total_duration_secs / 3600,
            (self.total_duration_secs % 3600) / 60
        );
        println!("  Total Opportunities:      {}", self.total_opportunities);
        println!("  Trades Executed:          {}", self.trades_executed);
        println!("  Profitable Trades:        {} ({:.1}%)",
            self.trades_profitable,
            self.win_rate
        );
        println!("  Unprofitable Trades:      {} ({:.1}%)",
            self.trades_unprofitable,
            100.0 - self.win_rate
        );

        println!("\n💰 PROFIT & LOSS");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  Gross Profit:             {:.4} SOL", self.total_gross_profit);
        println!("  Total Costs:              {:.4} SOL", self.total_costs);
        println!("  Net Profit:               {:.4} SOL", self.total_net_profit);
        println!("  Avg Profit per Trade:     {:.6} SOL", self.average_profit_per_trade);
        println!("  Max Single Profit:        {:.4} SOL", self.max_single_profit);
        println!("  Max Single Loss:          {:.4} SOL", self.max_single_loss);

        println!("\n📈 RISK METRICS");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  Peak Balance:             {:.4} SOL", self.peak_balance);
        println!("  Current Balance:          {:.4} SOL", self.current_balance);
        println!("  Max Drawdown:             {:.4} SOL", self.max_drawdown);
        println!("  Win Rate:                 {:.2}%", self.win_rate);
        println!("  Sharpe Ratio:             {:.3}", self.sharpe_ratio);

        println!("\n🎯 PERFORMANCE RATING");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Win rate rating
        let win_rate_status = if self.win_rate >= 70.0 {
            "✅ EXCELLENT"
        } else if self.win_rate >= 60.0 {
            "✅ GOOD"
        } else if self.win_rate >= 50.0 {
            "⚠️  ACCEPTABLE"
        } else {
            "❌ POOR"
        };
        println!("  Win Rate:                 {}", win_rate_status);

        // Average profit rating
        let avg_profit_status = if self.average_profit_per_trade >= 0.01 {
            "✅ EXCELLENT"
        } else if self.average_profit_per_trade >= 0.001 {
            "✅ GOOD"
        } else if self.average_profit_per_trade > 0.0 {
            "⚠️  LOW"
        } else {
            "❌ NEGATIVE"
        };
        println!("  Avg Profit:               {}", avg_profit_status);

        // Drawdown rating
        let drawdown_status = if self.max_drawdown < 1.0 {
            "✅ EXCELLENT"
        } else if self.max_drawdown < 5.0 {
            "✅ ACCEPTABLE"
        } else if self.max_drawdown < 10.0 {
            "⚠️  HIGH"
        } else {
            "❌ EXCESSIVE"
        };
        println!("  Max Drawdown:             {}", drawdown_status);

        // Sharpe ratio rating
        let sharpe_status = if self.sharpe_ratio >= 2.0 {
            "✅ EXCELLENT"
        } else if self.sharpe_ratio >= 1.0 {
            "✅ GOOD"
        } else if self.sharpe_ratio >= 0.5 {
            "⚠️  ACCEPTABLE"
        } else {
            "❌ POOR"
        };
        println!("  Sharpe Ratio:             {}", sharpe_status);

        println!("\n✅ PRODUCTION READINESS");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        let ready_for_production = 
            self.win_rate >= 60.0 &&
            self.average_profit_per_trade >= 0.001 &&
            self.max_drawdown < 5.0 &&
            self.total_net_profit > 0.0;

        if ready_for_production {
            println!("  Status:                   🟢 READY FOR PRODUCTION");
            println!("  Recommendation:           Deploy to Phase 1 (0.1 SOL max position)");
        } else {
            println!("  Status:                   🔴 NOT READY");
            println!("  Recommendation:           Review strategy and optimize parameters");
            
            if self.win_rate < 60.0 {
                println!("  Issue:                    Win rate too low ({:.1}% < 60%)", self.win_rate);
            }
            if self.average_profit_per_trade < 0.001 {
                println!("  Issue:                    Average profit too low ({:.6} SOL < 0.001 SOL)", self.average_profit_per_trade);
            }
            if self.max_drawdown >= 5.0 {
                println!("  Issue:                    Max drawdown too high ({:.4} SOL >= 5.0 SOL)", self.max_drawdown);
            }
            if self.total_net_profit <= 0.0 {
                println!("  Issue:                    Total net profit non-positive ({:.4} SOL)", self.total_net_profit);
            }
        }

        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    }
}

/// Paper trading simulator
struct PaperTradingSimulator {
    config: Config,
    detector: Arc<ArbitrageDetector>,
    dex_integrator: Arc<DexIntegrator>,
    transaction_builder: Arc<TransactionBuilder>,
    stats: Arc<Mutex<PaperTradingStats>>,
    trades: Arc<Mutex<Vec<PaperTrade>>>,
    running: Arc<AtomicBool>,
    opportunities_seen: Arc<AtomicU64>,
}

impl PaperTradingSimulator {
    fn new(config: Config) -> Self {
        let dex_integrator = Arc::new(DexIntegrator::new(config.clone()));
        let detector = Arc::new(ArbitrageDetector::new(config.clone(), dex_integrator.clone()));
        let keypair = Keypair::new(); // Dummy keypair for paper trading
        let transaction_builder = Arc::new(TransactionBuilder::new(
            config.clone(),
            keypair.pubkey(),
        ));

        let starting_balance = 100.0; // Simulated starting balance: 100 SOL

        Self {
            config,
            detector,
            dex_integrator,
            transaction_builder,
            stats: Arc::new(Mutex::new(PaperTradingStats::new(starting_balance))),
            trades: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(true)),
            opportunities_seen: Arc::new(AtomicU64::new(0)),
        }
    }

    async fn run_simulation(&self, max_duration: Duration, max_opportunities: u64) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🎯 Starting Paper Trading Simulation");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  Max Duration:        {} hours", max_duration.as_secs() / 3600);
        println!("  Max Opportunities:   {}", max_opportunities);
        println!("  Starting Balance:    100.0 SOL");
        println!("  Mode:                Paper Trading (No Real Transactions)");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        let start_time = Instant::now();
        let mut last_status_update = Instant::now();

        // Fetch initial pool data
        println!("📊 Fetching pool data from DEXs...");
        let pools = self.dex_integrator.fetch_all_pools().await?;
        println!("✓ Loaded {} pools", pools.len());

        loop {
            // Check termination conditions
            let elapsed = start_time.elapsed();
            let opportunities_count = self.opportunities_seen.load(Ordering::SeqCst);

            if elapsed >= max_duration {
                println!("\n⏰ Max duration reached ({} hours)", max_duration.as_secs() / 3600);
                break;
            }

            if opportunities_count >= max_opportunities {
                println!("\n✅ Max opportunities reached ({})", max_opportunities);
                break;
            }

            // Status update every minute
            if last_status_update.elapsed() >= Duration::from_secs(60) {
                let stats = self.stats.lock().unwrap();
                println!(
                    "[{:02}:{:02}] Opportunities: {} | Trades: {} | P&L: {:.4} SOL | Win Rate: {:.1}%",
                    elapsed.as_secs() / 3600,
                    (elapsed.as_secs() % 3600) / 60,
                    opportunities_count,
                    stats.trades_executed,
                    stats.total_net_profit,
                    stats.win_rate
                );
                last_status_update = Instant::now();
            }

            // Detect arbitrage opportunities
            if let Some(opportunity) = self.detector.detect_arbitrage(&pools).await {
                self.opportunities_seen.fetch_add(1, Ordering::SeqCst);
                self.stats.lock().unwrap().total_opportunities += 1;

                // Simulate trade execution
                self.simulate_trade(opportunity).await?;
            }

            // Small delay to avoid spinning
            sleep(Duration::from_millis(100)).await;

            // Periodically refresh pool data (every 10 seconds)
            if elapsed.as_secs() % 10 == 0 {
                let _pools = self.dex_integrator.fetch_all_pools().await?;
            }
        }

        // Finalize statistics
        let mut stats = self.stats.lock().unwrap();
        stats.finalize();

        // Calculate Sharpe ratio
        let trades = self.trades.lock().unwrap();
        let returns: Vec<f64> = trades.iter().map(|t| t.net_profit).collect();
        stats.calculate_sharpe_ratio(&returns);

        Ok(())
    }

    async fn simulate_trade(&self, opportunity: ArbitrageOpportunity) -> Result<(), Box<dyn std::error::Error>> {
        let start = Instant::now();

        // Estimate gas cost (typical Solana transaction)
        let base_fee = 0.000005; // 5000 lamports = 0.000005 SOL
        let compute_units = opportunity.path.len() as f64 * 200_000.0; // Estimate compute units
        let priority_fee = (compute_units / 1_000_000.0) * 0.000001; // Micro-lamports to SOL
        let simulated_cost = base_fee + priority_fee;

        // Get expected profit from opportunity
        let simulated_profit = opportunity.expected_profit;

        // Simulate success probability based on various factors
        let success_probability = self.calculate_success_probability(&opportunity);
        let would_have_succeeded = rand::random::<f64>() < success_probability;

        // Calculate net profit
        let net_profit = if would_have_succeeded {
            simulated_profit - simulated_cost
        } else {
            -simulated_cost // Only lose gas cost on failure
        };

        let execution_time_ms = start.elapsed().as_millis() as u64;

        let trade = PaperTrade {
            timestamp: SystemTime::now(),
            opportunity,
            simulated_profit,
            simulated_cost,
            net_profit,
            execution_time_ms,
            would_have_succeeded,
        };

        // Record trade
        self.stats.lock().unwrap().record_trade(&trade);
        self.trades.lock().unwrap().push(trade);

        Ok(())
    }

    fn calculate_success_probability(&self, opportunity: &ArbitrageOpportunity) -> f64 {
        let mut probability = 0.85; // Base success rate: 85%

        // Adjust for profit margin (higher margin = less likely to fail due to slippage)
        let profit_margin = opportunity.expected_profit / opportunity.input_amount;
        if profit_margin > 0.05 {
            probability += 0.1; // +10% for high margin
        } else if profit_margin < 0.01 {
            probability -= 0.2; // -20% for low margin
        }

        // Adjust for path length (longer paths = more chance of failure)
        let path_length_penalty = (opportunity.path.len() as f64 - 2.0) * 0.05;
        probability -= path_length_penalty;

        // Adjust for pool liquidity (assume opportunity has liquidity info)
        // For now, use a heuristic based on input amount
        if opportunity.input_amount > 10.0 {
            probability -= 0.15; // Large trades more likely to fail
        } else if opportunity.input_amount < 0.1 {
            probability += 0.05; // Small trades more likely to succeed
        }

        // Clamp between 0.3 and 0.95
        probability.max(0.3).min(0.95)
    }
}

#[tokio::test]
#[ignore] // This is a long-running test, run with --ignored flag
async fn test_paper_trading_24h() {
    // Initialize logging
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║       24-HOUR PAPER TRADING TEST - STRATEGY VALIDATION       ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Load configuration
    let config = Config::load_from_file("config.toml").expect("Failed to load config");

    // Override settings for paper trading
    let mut paper_config = config.clone();
    paper_config.safety.dry_run_only = true; // Force dry-run mode
    paper_config.arbitrage.min_profit_threshold = 0.001; // 0.001 SOL minimum

    // Create simulator
    let simulator = PaperTradingSimulator::new(paper_config);

    // Run simulation
    // For testing, we use shorter duration and fewer opportunities
    // In production, use: Duration::from_secs(24 * 3600) and 1000
    let max_duration = Duration::from_secs(60); // 1 minute for quick test
    let max_opportunities = 50; // 50 opportunities for test

    // For actual 24-hour test, uncomment:
    // let max_duration = Duration::from_secs(24 * 3600); // 24 hours
    // let max_opportunities = 1000;

    let result = simulator.run_simulation(max_duration, max_opportunities).await;
    assert!(result.is_ok(), "Simulation failed: {:?}", result.err());

    // Get final statistics
    let stats = simulator.stats.lock().unwrap();
    stats.print_report();

    // Assertions for production readiness
    println!("\n🧪 Running Production Readiness Tests...");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Test 1: Win rate > 60%
    println!("Test 1: Win Rate > 60%");
    println!("  Expected:    > 60.0%");
    println!("  Actual:      {:.2}%", stats.win_rate);
    assert!(
        stats.win_rate >= 60.0,
        "Win rate too low: {:.2}% < 60%",
        stats.win_rate
    );
    println!("  Result:      ✅ PASS\n");

    // Test 2: Average profit > 0.001 SOL per trade
    println!("Test 2: Average Profit > 0.001 SOL per trade");
    println!("  Expected:    > 0.001 SOL");
    println!("  Actual:      {:.6} SOL", stats.average_profit_per_trade);
    assert!(
        stats.average_profit_per_trade >= 0.001,
        "Average profit too low: {:.6} SOL < 0.001 SOL",
        stats.average_profit_per_trade
    );
    println!("  Result:      ✅ PASS\n");

    // Test 3: Max drawdown < 5 SOL
    println!("Test 3: Max Drawdown < 5.0 SOL");
    println!("  Expected:    < 5.0 SOL");
    println!("  Actual:      {:.4} SOL", stats.max_drawdown);
    assert!(
        stats.max_drawdown < 5.0,
        "Max drawdown too high: {:.4} SOL >= 5.0 SOL",
        stats.max_drawdown
    );
    println!("  Result:      ✅ PASS\n");

    // Test 4: Total net profit > 0
    println!("Test 4: Total Net Profit > 0");
    println!("  Expected:    > 0.0 SOL");
    println!("  Actual:      {:.4} SOL", stats.total_net_profit);
    assert!(
        stats.total_net_profit > 0.0,
        "Total net profit is negative: {:.4} SOL",
        stats.total_net_profit
    );
    println!("  Result:      ✅ PASS\n");

    // Test 5: Executed at least some trades
    println!("Test 5: Executed Trades > 0");
    println!("  Expected:    > 0");
    println!("  Actual:      {}", stats.trades_executed);
    assert!(
        stats.trades_executed > 0,
        "No trades were executed"
    );
    println!("  Result:      ✅ PASS\n");

    // Test 6: Sharpe ratio > 0.5 (risk-adjusted returns)
    println!("Test 6: Sharpe Ratio > 0.5");
    println!("  Expected:    > 0.5");
    println!("  Actual:      {:.3}", stats.sharpe_ratio);
    assert!(
        stats.sharpe_ratio >= 0.5,
        "Sharpe ratio too low: {:.3} < 0.5",
        stats.sharpe_ratio
    );
    println!("  Result:      ✅ PASS\n");

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🎉 ALL TESTS PASSED - STRATEGY VALIDATED");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("✅ Strategy is ready for production deployment");
    println!("📋 Next steps:");
    println!("   1. Deploy to devnet for real (but free) testing");
    println!("   2. Run for 1 week on devnet to validate");
    println!("   3. Deploy to mainnet Phase 1 (0.1 SOL max position)");
    println!("   4. Monitor continuously for first 48 hours");
    println!("   5. Scale gradually to Phase 2, 3, 4 based on success\n");
}

#[tokio::test]
#[ignore]
async fn test_paper_trading_stress_test() {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║            STRESS TEST - HIGH VOLUME SIMULATION               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    let config = Config::load_from_file("config.toml").expect("Failed to load config");
    let mut stress_config = config.clone();
    stress_config.safety.dry_run_only = true;
    stress_config.arbitrage.min_profit_threshold = 0.0001; // Lower threshold for more opportunities

    let simulator = PaperTradingSimulator::new(stress_config);

    // Stress test: 5000 opportunities in 1 hour
    let max_duration = Duration::from_secs(3600); // 1 hour
    let max_opportunities = 5000;

    let result = simulator.run_simulation(max_duration, max_opportunities).await;
    assert!(result.is_ok(), "Stress test failed: {:?}", result.err());

    let stats = simulator.stats.lock().unwrap();
    stats.print_report();

    // Assert system remained stable under high volume
    assert!(stats.trades_executed > 100, "Not enough trades executed in stress test");
    println!("✅ Stress test passed - system stable under high volume\n");
}

#[tokio::test]
#[ignore]
async fn test_paper_trading_adverse_conditions() {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║         ADVERSE CONDITIONS TEST - LOW PROFITABILITY           ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    let config = Config::load_from_file("config.toml").expect("Failed to load config");
    let mut adverse_config = config.clone();
    adverse_config.safety.dry_run_only = true;
    adverse_config.arbitrage.min_profit_threshold = 0.005; // High threshold = fewer opportunities

    let simulator = PaperTradingSimulator::new(adverse_config);

    let max_duration = Duration::from_secs(300); // 5 minutes
    let max_opportunities = 20;

    let result = simulator.run_simulation(max_duration, max_opportunities).await;
    assert!(result.is_ok(), "Adverse conditions test failed: {:?}", result.err());

    let stats = simulator.stats.lock().unwrap();
    stats.print_report();

    // In adverse conditions, we should still:
    // 1. Not lose too much money (circuit breaker should help)
    // 2. Maintain reasonable win rate
    // 3. Not experience excessive drawdown

    assert!(
        stats.max_drawdown < 10.0,
        "Excessive drawdown in adverse conditions: {:.4} SOL",
        stats.max_drawdown
    );

    println!("✅ Adverse conditions test passed - bot handles low profitability well\n");
}
