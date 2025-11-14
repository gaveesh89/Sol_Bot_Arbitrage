# Triangular Arbitrage Detection Module

## Overview

The triangular arbitrage module implements a graph-based approach to detect profitable trading cycles across multiple Solana DEXs (Raydium, Meteora, Pump, Whirlpool, and Orca).

## Algorithm

### Graph Representation

The module uses a directed weighted graph where:
- **Nodes**: Token mint addresses (Solana Pubkeys)
- **Edges**: Exchange rates between token pairs on specific DEXs
- **Weights**: Negative log-transformed rates: `weight = -log(rate × (1 - fee))`

### Why Negative Log Weights?

Using negative logarithms transforms multiplicative operations into additive ones:

```
Profit calculation:
- Direct: output = input × rate₁ × rate₂ × rate₃
- Log-transformed: log(output/input) = log(rate₁) + log(rate₂) + log(rate₃)
- Negative: -log(output/input) = -log(rate₁) - log(rate₂) - log(rate₃)

A negative cycle weight means: -log(output/input) < 0
Which implies: output/input > 1 (PROFIT!)
```

### Arbitrage Detection

The algorithm uses breadth-first search (BFS) to find cycles of length 2-3 that return to the starting token:

1. Start from a token (e.g., USDC)
2. Explore all possible paths up to max length (default: 3)
3. When a path returns to the start token, check if cycle weight < 0
4. If negative, calculate profit: `profit_ratio = exp(-cycle_weight)`

## Core Components

### ExchangeEdge

Represents a single trading route between two tokens on a specific DEX:

```rust
pub struct ExchangeEdge {
    pub from_token: Pubkey,
    pub to_token: Pubkey,
    pub dex: DexType,
    pub pool_address: Pubkey,
    pub rate: f64,                    // Exchange rate
    pub inverse_log_weight: f64,      // -log(rate × (1 - fee))
    pub liquidity_depth: Vec<PriceLevel>,
    pub fee_bps: u16,                 // Fee in basis points
    pub last_update: i64,
}
```

### ArbitrageGraph

Main graph structure with efficient adjacency list representation:

```rust
pub struct ArbitrageGraph {
    adjacency: HashMap<Pubkey, Vec<ExchangeEdge>>,
    edge_lookup: HashMap<(Pubkey, Pubkey, DexType), (usize, usize)>,
    tokens: HashSet<Pubkey>,
}
```

**Key Methods:**
- `add_edge()`: Add new trading route
- `update_edge_rate()`: Update exchange rate for existing edge
- `detect_triangular_arbitrage()`: Find profitable cycles from specific token
- `detect_all_triangular_arbitrage()`: Find all profitable cycles
- `calculate_optimal_trade_size()`: Determine best trade amount considering liquidity

### TriangularArbitrageOpportunity

Represents a detected profitable cycle:

```rust
pub struct TriangularArbitrageOpportunity {
    pub path: Vec<ExchangeEdge>,      // Sequence of trades
    pub profit_ratio: f64,             // e.g., 1.02 = 2% profit
    pub profit_bps: i64,               // Profit in basis points
    pub input_token: Pubkey,
    pub input_amount: u64,
    pub estimated_output: u64,
    pub total_fees_bps: u16,
    pub cycle_weight: f64,             // Negative if profitable
}
```

## Usage Examples

### 1. Create and Populate Graph

```rust
use solana_mev_bot::dex::triangular_arb::*;

let mut graph = ArbitrageGraph::new();

// Add USDC -> SOL edge on Raydium
let edge = ExchangeEdge::new(
    usdc_mint,
    sol_mint,
    DexType::Raydium,
    pool_address,
    0.01234,  // rate: 1 USDC = 0.01234 SOL
    25,       // 0.25% fee
    vec![],   // liquidity depth
    current_timestamp,
);
graph.add_edge(edge);

// Add more edges...
```

### 2. Detect Arbitrage Opportunities

```rust
// Find triangular arbitrage starting from USDC
let opportunities = graph.detect_triangular_arbitrage(
    &usdc_mint,
    3,      // max path length
    50,     // min 50 bps (0.5%) profit
);

for opp in opportunities {
    println!("Profit: {} bps", opp.profit_bps);
    println!("Path: {} hops", opp.path.len());
    
    // Check if profitable after gas
    if opp.is_profitable_after_costs(gas_cost, token_price) {
        // Execute trade...
    }
}
```

### 3. Thread-Safe Concurrent Access

```rust
use solana_mev_bot::dex::triangular_arb::create_shared_graph;

let graph = create_shared_graph();

// Writer thread
{
    let mut g = graph.write().unwrap();
    g.add_edge(edge);
}

// Reader threads
{
    let g = graph.read().unwrap();
    let opportunities = g.detect_all_triangular_arbitrage(3, 50);
}
```

### 4. Calculate Optimal Trade Size

```rust
let (optimal_input, expected_output) = graph.calculate_optimal_trade_size(
    &opportunity,
    max_trade_amount,
    100,  // max 1% slippage
)?;

println!("Trade {} for {} (profit: {})", 
    optimal_input, 
    expected_output,
    expected_output - optimal_input
);
```

## Example Triangular Arbitrage

Consider this scenario:

```
USDC -> SOL (Raydium):  1 USDC = 0.01 SOL    (fee: 0.25%)
SOL -> BONK (Meteora):  1 SOL = 1M BONK      (fee: 0.30%)
BONK -> USDC (Orca):    1M BONK = 1.02 USDC  (fee: 0.25%)
```

**Calculation:**
1. Start: 1000 USDC
2. USDC → SOL: 1000 × 0.01 × (1 - 0.0025) = 9.975 SOL
3. SOL → BONK: 9.975 × 1,000,000 × (1 - 0.003) = 9,945,075 BONK
4. BONK → USDC: 9,945,075 / 1,000,000 × 1.02 × (1 - 0.0025) = 1012.07 USDC

**Profit:** 12.07 USDC (1.207% = 120.7 bps) ✅

## Performance Considerations

### Time Complexity
- **Add edge**: O(1)
- **Update edge**: O(1)
- **Detect from single token**: O(V + E) where V = tokens, E = edges
- **Detect all**: O(V × (V + E))

### Space Complexity
- **Graph storage**: O(V + E)
- **Path exploration**: O(V × max_path_length)

### Optimizations
1. **Edge lookup table**: O(1) edge updates
2. **Early termination**: Stop exploring unprofitable paths
3. **Liquidity filtering**: Skip edges with insufficient depth
4. **Profit sorting**: Return best opportunities first

## Testing

The module includes comprehensive tests:

```bash
# Run all triangular arbitrage tests
cargo test triangular_arb

# Run specific test
cargo test test_triangular_arbitrage_detection
```

**Test Coverage:**
- ✅ Weight calculation correctness
- ✅ Edge addition and updates
- ✅ Profitable cycle detection
- ✅ No false positives (fees eat profit)
- ✅ Thread-safe concurrent access
- ✅ Optimal trade size calculation

## Integration with MEV Bot

### 1. Initialize Graph on Startup

```rust
use solana_mev_bot::dex::triangular_arb::*;

let arb_graph = create_shared_graph();

// Populate with pool data from all DEXs
for pool in all_pools {
    let edge = create_edge_from_pool(pool);
    arb_graph.write().unwrap().add_edge(edge);
}
```

### 2. Update Prices Continuously

```rust
// In price monitoring loop
for update in price_updates {
    arb_graph.write().unwrap().update_edge_rate(
        update.from_token,
        update.to_token,
        update.dex,
        update.new_rate,
        current_timestamp,
    )?;
}
```

### 3. Detect and Execute

```rust
// Periodic arbitrage detection
let opportunities = arb_graph.read().unwrap()
    .detect_all_triangular_arbitrage(3, 50);

for opp in opportunities {
    if should_execute(&opp) {
        execute_triangular_arbitrage(opp).await?;
    }
}
```

## Configuration

Recommended settings:

```bash
# In .env or config
TRIANGULAR_ARB_ENABLED=true
TRIANGULAR_MAX_PATH_LENGTH=3        # 2 or 3 hops recommended
TRIANGULAR_MIN_PROFIT_BPS=50        # 0.5% minimum
TRIANGULAR_MAX_SLIPPAGE_BPS=100     # 1% max slippage
TRIANGULAR_CHECK_INTERVAL_MS=1000   # Check every second
```

## Monitoring and Logging

The module provides detailed logging:

```
INFO: Initializing ArbitrageGraph for triangular arbitrage detection
DEBUG: Added edge: USDC -> SOL via Raydium (rate: 0.01234, weight: -2.456)
INFO: Found triangular arbitrage: profit=120 bps, path_length=3, cycle_weight=-0.012
```

## Known Limitations

1. **Static liquidity**: Assumes liquidity depth doesn't change during execution
2. **No MEV protection**: Detected opportunities may be frontrun
3. **Gas costs**: Must be factored in separately
4. **Slippage**: Real execution may differ from simulation
5. **Network latency**: Prices may change before execution

## Future Enhancements

- [ ] Dynamic liquidity tracking with real-time updates
- [ ] MEV protection via private transaction submission
- [ ] Multi-threaded parallel opportunity detection
- [ ] Machine learning for profitability prediction
- [ ] Historical profitability tracking and analytics
- [ ] Automatic gas cost optimization
- [ ] Flash loan integration for larger trades

## References

- [Bellman-Ford Algorithm](https://en.wikipedia.org/wiki/Bellman%E2%80%93Ford_algorithm)
- [Negative Cycle Detection](https://www.geeksforgeeks.org/detect-negative-cycle-graph-bellman-ford/)
- [Arbitrage in DeFi](https://arxiv.org/abs/2105.02784)
- [Graph-Based Trading Strategies](https://quantpedia.com/arbitrage-strategy/)

## Support

For questions or issues:
- File an issue on GitHub
- Check existing test cases for usage examples
- Review the inline documentation in `src/dex/triangular_arb.rs`
