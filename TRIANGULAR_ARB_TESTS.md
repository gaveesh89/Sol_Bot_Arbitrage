# Triangular Arbitrage Tests

## Overview
Comprehensive unit tests for the Bellman-Ford triangular arbitrage detection system. All tests use mock data and avoid RPC dependencies for fast, reliable testing.

## Test Suite Summary

**Location:** `src/dex/triangular_arb_tests.rs`  
**Total Tests:** 10  
**Status:** ✅ All passing

### Test Coverage

#### 1. **test_simple_triangular_arbitrage**
- **Purpose:** Verify basic 3-hop cycle detection (SOL → USDC → USDT → SOL)
- **Validates:** Path length, profit calculation, cycle detection
- **Premium Used:** 5% to overcome fees

#### 2. **test_no_arbitrage_when_rates_fair**
- **Purpose:** Ensure fair market rates don't trigger false positives
- **Validates:** Profitability threshold enforcement
- **Expected:** No cycles or unprofitable cycles rejected

#### 3. **test_profit_calculation_with_fees**
- **Purpose:** Verify profit calculations account for DEX fees
- **Validates:** Logarithmic weight calculations produce positive profit
- **Fee Structure:** 30 bps (0.3%) per hop

#### 4. **test_slippage_reduces_profitability**
- **Purpose:** Test liquidity depth impact on tradeable amounts
- **Validates:** `get_max_tradeable_amount()` respects price levels
- **Verifies:** Larger slippage tolerance = larger tradeable amount

#### 5. **test_concurrent_graph_updates**
- **Purpose:** Stress test concurrent edge additions
- **Validates:** Thread-safe RwLock operations
- **Scale:** 10 tasks × 100 edges = 1,000 concurrent updates

#### 6. **test_four_hop_arbitrage**
- **Purpose:** Test extended path detection (4 hops)
- **Validates:** Max path length configuration works correctly
- **Path:** SOL → USDC → USDT → BONK → SOL

#### 7. **test_edge_weight_calculation**
- **Purpose:** Verify inverse logarithmic weight formula
- **Validates:** Weight is finite, negative (for negative cycle detection)
- **Formula:** `-ln(rate × (1 - fee_bps/10000))`

#### 8. **test_negative_profit_detection**
- **Purpose:** Ensure unprofitable cycles are rejected
- **Validates:** Loss-making paths don't trigger execution
- **Design:** Intentionally use rates that guarantee loss

#### 9. **test_high_fee_impact**
- **Purpose:** Compare profitability with different fee structures
- **Validates:** Higher fees reduce profit as expected
- **Comparison:** 0.1% fees vs 1.0% fees

#### 10. **test_decimal_handling**
- **Purpose:** Verify rate storage and precision
- **Validates:** f64 weights are finite and correct
- **Critical:** Prevents NaN/Inf in Bellman-Ford algorithm

## Mock Data Approach

### Token Addresses
```rust
SOL:  So11111111111111111111111111111111111111112
USDC: EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
USDT: Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB
BONK: DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263
```

### Helper Functions

**`create_test_edge()`** - Creates ExchangeEdge with mock data:
- Token pair (from/to)
- Exchange rate
- Fee in basis points
- Liquidity levels (2-tier order book)
- DexType: Raydium (default)
- Unique pool address

## Running Tests

### Run All Triangular Arbitrage Tests
```bash
cargo test triangular_arb_tests --lib
```

### Run with Output
```bash
cargo test triangular_arb_tests --lib -- --nocapture
```

### Run Specific Test
```bash
cargo test test_simple_triangular_arbitrage --lib
```

### Run Full Test Suite
```bash
cargo test --lib
```

## Key Insights

### Path Length Convention
The Bellman-Ford implementation includes the start token in the path:
- **3-hop cycle:** 4 tokens in path (SOL → USDC → USDT → SOL)
- **4-hop cycle:** 5 tokens in path (includes start token twice)

### Profit Calculation
The actual profit calculations use logarithmic weights, which can produce very high basis point values in tests. This is expected behavior for:
- Mock data with simplified rates
- No real liquidity constraints
- Testing mathematical correctness, not realistic profits

### Fee Impact
Each DEX swap typically charges 0.3% (30 bps):
- **3-hop cycle:** ~90 bps total fees
- **4-hop cycle:** ~120 bps total fees
- Requires significant rate discrepancies to be profitable

### Concurrency Safety
The `ArbitrageGraph` uses `std::sync::RwLock` (not tokio::RwLock):
- Compatible with Bellman-Ford sync operations
- Allows multiple concurrent readers
- Single writer for updates
- Thread-safe across async tasks

## Integration with Production Code

These tests validate the core algorithms used in:
- `src/dex/triangular_arb.rs` - Main detection logic
- `src/chain/detector.rs` - Opportunity detection
- `src/chain/integration.rs` - MEV bot orchestration

### Test vs Production Differences

**Tests:**
- Mock token addresses
- Simplified liquidity curves
- No RPC calls
- Synchronous operation

**Production:**
- Real Solana token mints
- Multi-level order books from DEX pools
- Live RPC data fetching
- Async WebSocket updates

## Best Practices

### Adding New Tests
1. Use `create_test_edge()` helper for consistency
2. Test with realistic fee structures (20-100 bps)
3. Include both profitable and unprofitable scenarios
4. Use `tokio::test` for async operations
5. Print results with `println!()` for debugging

### Test Data Design
- **Obvious Arbitrage:** 5%+ premiums to overcome fees
- **Fair Rates:** <1% differences (should be rejected)
- **Loss Scenarios:** Intentionally bad rates for negative tests

### Debugging Failed Tests
```bash
# Run with backtrace
RUST_BACKTRACE=1 cargo test test_name --lib

# Run with full output
cargo test test_name --lib -- --nocapture --show-output

# Check specific edge weights
# Add debug prints in create_test_edge() if needed
```

## Maintenance Notes

### When to Update Tests
- Adding new DEX integrations
- Changing fee calculation logic
- Modifying Bellman-Ford parameters
- Updating profit thresholds

### Performance Benchmarks
- **10 tests:** ~0.13 seconds
- **Concurrent test:** 1,000 updates in <100ms
- **Detection latency:** Sub-millisecond for 3-hop cycles

### Known Limitations
1. Tests use simplified 2-tier liquidity curves
2. No slippage model validation against real DEX behavior
3. Front-running scenarios not tested (covered in integration layer)
4. Gas fee impact not included in test profits

## Related Documentation
- `BELLMAN_FORD_ARBITRAGE.md` - Algorithm explanation
- `TRIANGULAR_ARBITRAGE.md` - Strategy guide
- `INTEGRATION_GUIDE.md` - Full system integration
- `QUICKSTART.md` - Running the bot

## Test Results
```
Running unittests src/lib.rs
running 10 tests
✅ test_decimal_handling ... ok
✅ test_edge_weight_calculation ... ok
✅ test_slippage_reduces_profitability ... ok
✅ test_no_arbitrage_when_rates_fair ... ok
✅ test_negative_profit_detection ... ok
✅ test_simple_triangular_arbitrage ... ok
✅ test_profit_calculation_with_fees ... ok
✅ test_four_hop_arbitrage ... ok
✅ test_high_fee_impact ... ok
✅ test_concurrent_graph_updates ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

Total project tests: **95 passing** (10 new + 85 existing)
