# Arbitrage Detection Orchestrator

## Overview

The Arbitrage Detection Orchestrator (`ArbitrageDetector`) is the main coordinator that runs continuous arbitrage detection across the trading graph. It manages the complete pipeline from cycle detection to opportunity evaluation and execution handoff.

## Architecture

### Components

**ArbitrageDetector**
- Coordinates Bellman-Ford detection across multiple base tokens
- Calculates optimal trade sizes with slippage consideration
- Evaluates and scores opportunities
- Sends profitable opportunities to execution engine
- Tracks performance metrics

**ArbitrageOpportunity**
- Complete opportunity ready for execution
- Includes optimal input amount, expected output, profit calculations
- Priority score and risk assessment

**DetectionMetrics**
- Total detections run
- Opportunities found and sent
- Average detection latency
- Per-token profitability statistics

## Key Features

### 1. Multi-Base Token Detection

Runs detection from high-liquidity base tokens:
- **SOL**: Native token, highest liquidity
- **USDC**: Most liquid stablecoin
- **USDT**: Second stablecoin

```rust
let detector = ArbitrageDetector::new(graph, min_profit_bps, opportunity_tx);
// Automatically initialized with SOL, USDC, USDT
```

### 2. Slippage-Adjusted Profitability

Calculates realistic profit after slippage:

```rust
// Considers:
// - Pool liquidity depth
// - Trade size relative to liquidity
// - Estimated slippage (2% default)
let (optimal_input, expected_output, profit_sol) = 
    detector.calculate_optimal_input(&cycle).await?;
```

### 3. Opportunity Prioritization

Scores opportunities 0.0 to 1.0 based on:
- **40% - Expected Profit**: Higher profit = higher score
- **30% - Path Length**: Fewer hops = higher score (2 hops = 1.0, 4 hops = 0.6)
- **20% - Liquidity Depth**: Higher liquidity = better execution
- **10% - DEX Reliability**: Raydium/Orca = 1.0, Pump = 0.7

```rust
opportunity.priority_score = detector.calculate_priority_score(&opportunity);
// Range: 0.0 to 1.0
```

### 4. Risk Assessment

Classifies opportunities into risk levels:

| Risk Level | Criteria | Example |
|------------|----------|---------|
| **Low** | 2-3 hops, >2% profit | SOL → USDC → SOL, 2.5% profit |
| **Medium** | 3 hops, 0.5-2% profit | SOL → USDC → RAY → SOL, 1.2% profit |
| **High** | 4+ hops or <0.5% profit | Complex 4-hop cycle, 0.3% profit |

### 5. Performance Metrics

Tracks detection performance:

```rust
let metrics = detector.get_metrics().await;
println!("Total detections: {}", metrics.total_detections);
println!("Opportunities found: {}", metrics.opportunities_found);
println!("Avg latency: {:.2}ms", metrics.avg_detection_latency_ms);
```

## Usage

### Basic Setup

```rust
use tokio::sync::mpsc;
use crate::chain::detector::ArbitrageDetector;
use crate::dex::triangular_arb::create_shared_graph;

#[tokio::main]
async fn main() -> Result<()> {
    // Create graph
    let graph = create_shared_graph();
    
    // Channel for sending opportunities to execution
    let (opportunity_tx, mut opportunity_rx) = mpsc::unbounded_channel();
    
    // Create detector with 1% minimum profit (100 bps)
    let detector = ArbitrageDetector::new(
        graph.clone(),
        100,  // 1% min profit
        opportunity_tx,
    );
    
    // Create update signal channel
    let (update_tx, update_rx) = mpsc::unbounded_channel();
    
    // Spawn detection loop
    tokio::spawn(async move {
        detector.run_detection_loop(update_rx).await;
    });
    
    // When graph updates, signal detection
    update_tx.send(())?;
    
    // Receive opportunities
    while let Some(opp) = opportunity_rx.recv().await {
        println!(
            "Opportunity: {:.2}% profit, priority={:.2}, risk={:?}",
            opp.expected_profit_bps as f64 / 100.0,
            opp.priority_score,
            opp.risk_level
        );
    }
    
    Ok(())
}
```

### Integration with WebSocket Monitor

```rust
use crate::chain::pool_monitor::PoolMonitor;
use crate::chain::detector::ArbitrageDetector;

// Create update signal channel
let (update_tx, update_rx) = mpsc::unbounded_channel();

// Create detector with update receiver
let detector = ArbitrageDetector::new(graph.clone(), 100, opportunity_tx);
tokio::spawn(async move {
    detector.run_detection_loop(update_rx).await;
});

// In pool monitor, signal detection after graph updates
// (Modified pool_monitor to accept update_tx)
pool_monitor.set_update_signal(update_tx);
```

### Dynamic Configuration

```rust
// Add new base token
detector.add_base_token(wbtc_pubkey);

// Remove base token
detector.remove_base_token(&usdt_pubkey);

// Update minimum profit threshold
detector.set_min_profit(200); // 2%

// Update max path length
detector.set_max_path_length(3); // Only 2-3 hop cycles
```

## Detection Pipeline

### Flow

```
1. Graph Update Signal
   ↓
2. For Each Base Token (SOL, USDC, USDT)
   ↓
3. Run Bellman-Ford Detection
   ↓
4. Filter by Min Profit Threshold
   ↓
5. Calculate Optimal Input Amount (with slippage)
   ↓
6. Re-verify Profitability After Slippage
   ↓
7. Calculate Priority Score
   ↓
8. Assess Risk Level
   ↓
9. Send to Execution Engine
   ↓
10. Update Metrics
```

### Performance

**Latency Breakdown:**
- Bellman-Ford detection: 50-300ms (depends on graph size)
- Opportunity processing: 5-20ms per cycle
- **Total**: 100-500ms typical

**Throughput:**
- 2-5 detections per second (rate-limited by pool monitor)
- 0-10 opportunities per detection
- **Peak**: 50 opportunities/second

## Opportunity Structure

```rust
pub struct ArbitrageOpportunity {
    pub cycle: ArbitrageCycle,           // Complete cycle with path
    pub optimal_input_amount: u64,       // Lamports to trade
    pub expected_output_amount: u64,     // Expected lamports out
    pub expected_profit_sol: f64,        // Profit in SOL
    pub expected_profit_bps: i64,        // Profit in basis points
    pub detected_at: i64,                // Unix timestamp
    pub priority_score: f64,             // 0.0 to 1.0
    pub risk_level: RiskLevel,           // Low/Medium/High
}
```

### Example Opportunity

```
cycle: SOL → USDC → RAY → SOL
  Step 1: SOL → USDC (Raydium, rate=150.0, fee=0.25%)
  Step 2: USDC → RAY (Orca, rate=0.5, fee=0.30%)
  Step 3: RAY → SOL (Meteora, rate=0.0135, fee=0.25%)

optimal_input_amount: 100,000,000 (0.1 SOL)
expected_output_amount: 102,000,000 (0.102 SOL)
expected_profit_sol: 0.002 SOL (~$0.20 at $100/SOL)
expected_profit_bps: 200 (2%)
priority_score: 0.75
risk_level: Low
```

## Metrics and Monitoring

### Available Metrics

```rust
pub struct DetectionMetrics {
    pub total_detections: u64,              // Total detection runs
    pub opportunities_found: u64,           // Total opportunities detected
    pub opportunities_sent: u64,            // Sent to execution
    pub avg_detection_latency_ms: f64,      // Average detection time
    pub last_detection_time: Option<Instant>, // Last detection timestamp
    pub profitable_by_token: HashMap<String, u64>, // Per-token stats
}
```

### Accessing Metrics

```rust
let metrics = detector.get_metrics().await;

info!("Detection Performance:");
info!("  Total runs: {}", metrics.total_detections);
info!("  Opportunities: {}", metrics.opportunities_found);
info!("  Avg latency: {:.2}ms", metrics.avg_detection_latency_ms);
info!("  Success rate: {:.1}%", 
    100.0 * metrics.opportunities_found as f64 / metrics.total_detections as f64
);
```

## Configuration Recommendations

### Conservative (Learning/Testing)

```rust
ArbitrageDetector::new(
    graph,
    200,  // 2% minimum profit
    opportunity_tx,
);
detector.set_max_path_length(3); // Only 2-3 hops
```

**Characteristics:**
- Lower risk, higher confidence
- Fewer opportunities, but higher quality
- Good for testing execution pipeline

### Aggressive (Production)

```rust
ArbitrageDetector::new(
    graph,
    50,   // 0.5% minimum profit
    opportunity_tx,
);
detector.set_max_path_length(4); // Up to 4 hops
```

**Characteristics:**
- More opportunities
- Higher execution risk
- Requires robust error handling

### High-Frequency

```rust
ArbitrageDetector::new(
    graph,
    30,   // 0.3% minimum profit
    opportunity_tx,
);
// Add more base tokens
detector.add_base_token(ray_pubkey);
detector.add_base_token(bonk_pubkey);
```

**Characteristics:**
- Maximum opportunity capture
- Highest competition
- Requires fast execution (<500ms)

## Best Practices

### 1. Profit Thresholds

Consider transaction fees when setting minimum profit:

```
Solana transaction fee: ~0.000005 SOL (~$0.0005)
Priority fee (congestion): 0.00001-0.0001 SOL ($0.001-$0.01)
Total cost: ~$0.002-$0.015 per transaction

Minimum profitable trade at different sizes:
- 0.1 SOL trade: Need >2% profit ($0.02) to cover fees
- 1.0 SOL trade: Need >0.2% profit ($0.20) to cover fees
- 10 SOL trade: Need >0.02% profit ($2.00) to cover fees
```

**Recommendation**: Set `min_profit_bps` based on typical trade size:
- Small trades (0.1-1 SOL): 150-200 bps (1.5-2%)
- Medium trades (1-10 SOL): 50-100 bps (0.5-1%)
- Large trades (10+ SOL): 30-50 bps (0.3-0.5%)

### 2. Base Token Selection

Focus on tokens with:
- **High liquidity** (>$1M)
- **Many trading pairs** (>20 pools)
- **Stable prices** (low volatility)

### 3. Slippage Estimation

Current implementation uses fixed slippage (2%). For production:

```rust
// TODO: Implement dynamic slippage based on:
// - Real liquidity depth from graph
// - Historical slippage data
// - Pool-specific characteristics
```

### 4. Risk Management

Filter opportunities by risk level:

```rust
if opportunity.risk_level == RiskLevel::High {
    debug!("Skipping high-risk opportunity");
    continue;
}

// Or adjust trade size based on risk
let adjusted_input = match opportunity.risk_level {
    RiskLevel::Low => opportunity.optimal_input_amount,
    RiskLevel::Medium => opportunity.optimal_input_amount / 2,
    RiskLevel::High => opportunity.optimal_input_amount / 4,
};
```

## Future Enhancements

### Planned Features

1. **Dynamic Slippage Calculation**
   - Use real liquidity depth from graph
   - Historical slippage analysis per DEX/pool
   
2. **Opportunity Batching**
   - Detect multiple compatible opportunities
   - Execute multiple cycles in one transaction
   
3. **ML-Based Prioritization**
   - Learn from execution success rates
   - Adjust scoring based on historical performance
   
4. **Cross-Chain Opportunities**
   - Detect arbitrage across Solana bridges
   - Wormhole, Allbridge integration

5. **Gas Price Optimization**
   - Dynamic priority fee based on opportunity size
   - Skip opportunities if gas too high

## Troubleshooting

### No Opportunities Detected

**Possible causes:**
1. `min_profit_bps` too high → Lower threshold
2. Graph not updated → Verify WebSocket monitor running
3. No profitable cycles → Market efficiency (normal)

### Low Opportunity Quality

**Solutions:**
1. Increase base token diversity
2. Add more DEXs to pool fetcher
3. Reduce `max_path_length` for simpler cycles

### High Detection Latency

**Solutions:**
1. Reduce graph size (filter low-liquidity pools)
2. Optimize Bellman-Ford (already optimized)
3. Run detection on separate thread pool

### Execution Failures

**Common issues:**
1. Slippage higher than estimated → Increase slippage tolerance
2. Pool state changed → Faster execution (<500ms)
3. Insufficient balance → Check wallet funding

## Integration Example

Complete example with pool monitoring and execution:

```rust
use crate::chain::{detector::ArbitrageDetector, pool_monitor::PoolMonitor};
use crate::dex::{triangular_arb::create_shared_graph, pool_fetcher::PoolDataFetcher};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Create shared graph
    let graph = create_shared_graph();
    
    // 2. Fetch initial pools and populate graph
    let pool_fetcher = PoolDataFetcher::new(rpc_clients, 10_000);
    let pools = pool_fetcher.fetch_pools().await?;
    for pool in pools {
        // Add to graph...
    }
    
    // 3. Create channels
    let (opportunity_tx, mut opportunity_rx) = mpsc::unbounded_channel();
    let (update_tx, update_rx) = mpsc::unbounded_channel();
    
    // 4. Start detector
    let detector = ArbitrageDetector::new(graph.clone(), 100, opportunity_tx);
    tokio::spawn(async move {
        detector.run_detection_loop(update_rx).await;
    });
    
    // 5. Start WebSocket monitor (signals detection on updates)
    let monitor = PoolMonitor::new(config, pool_addresses, graph, update_tx);
    tokio::spawn(async move {
        monitor.start_monitoring().await
    });
    
    // 6. Execution loop
    while let Some(opp) = opportunity_rx.recv().await {
        match execute_opportunity(opp).await {
            Ok(signature) => info!("Executed: {}", signature),
            Err(e) => warn!("Execution failed: {}", e),
        }
    }
    
    Ok(())
}
```

## Conclusion

The Arbitrage Detector is the brain of the MEV bot, coordinating detection, evaluation, and opportunity handoff. Proper configuration of profit thresholds, base tokens, and risk parameters is crucial for profitability.

For more information, see:
- `BELLMAN_FORD_ARBITRAGE.md` - Detection algorithm details
- `WEBSOCKET_MONITORING.md` - Real-time graph updates
- `POOL_FETCHER_GUIDE.MD` - Pool data collection
