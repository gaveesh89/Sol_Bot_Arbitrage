# Memory Stability Test Guide

## Overview

The `test_memory_usage_stable()` integration test validates that the Bellman-Ford arbitrage detector doesn't have memory leaks and maintains stable memory usage during continuous operation.

## Test Location

**File:** `tests/integration_tests.rs`  
**Test Name:** `test_memory_usage_stable`

## What It Tests

### 1. Memory Leak Detection
- Simulates 1,000 pool updates
- Monitors heap memory usage every 100 updates
- Validates memory doesn't grow unbounded

### 2. Graph Memory Management
- Verifies graph clear/repopulate doesn't leak
- Checks that edge allocations are properly released
- Ensures detector buffers are reused correctly

### 3. Long-Running Stability
- Projects memory usage at 10k and 100k updates
- Calculates linear regression trend
- Identifies if memory growth is acceptable or concerning

## Running the Test

### Prerequisites

1. Install memory-stats dependency (already added to `Cargo.toml`)
2. Optional: Start mainnet fork validator for realistic conditions

```bash
# Start validator (optional, test works without it)
./start-mainnet-fork.sh
```

### Run Command

```bash
# Run memory stability test
cargo test --test integration_tests test_memory_usage_stable -- --ignored --nocapture
```

### Expected Runtime

- **Duration:** ~10-15 seconds
- **Pool Updates:** 1,000
- **Memory Checks:** 10 (every 100 updates)

## Test Output

### Example Successful Run

```
╔════════════════════════════════════════════════════════════════╗
║  🧪 TEST: Memory Usage Stability                              ║
╚════════════════════════════════════════════════════════════════╝

Validates that repeated pool updates don't cause memory leaks.
Simulates 1000 pool updates and monitors heap usage.

🔧 Setup
========

✅ Validator running: 1.18.26
✅ Created arbitrage graph and detector
✅ Minimum profit threshold: 10 bps

📊 Test Configuration
=====================

   • Total pool updates: 1000
   • Memory check interval: every 100 updates
   • Max allowed growth: 50.0 MB

🔬 Memory Stability Test
=========================

📌 Baseline memory: 245.32 MB

⚡ Running 1000 pool updates...
─────────────────────────────────────────

   100 / 1000 updates: 246.15 MB (growth: +0.83 MB)
   200 / 1000 updates: 246.42 MB (growth: +1.10 MB)
   300 / 1000 updates: 246.58 MB (growth: +1.26 MB)
   400 / 1000 updates: 246.71 MB (growth: +1.39 MB)
   500 / 1000 updates: 246.83 MB (growth: +1.51 MB)
   600 / 1000 updates: 246.94 MB (growth: +1.62 MB)
   700 / 1000 updates: 247.04 MB (growth: +1.72 MB)
   800 / 1000 updates: 247.13 MB (growth: +1.81 MB)
   900 / 1000 updates: 247.21 MB (growth: +1.89 MB)
   1000 / 1000 updates: 247.28 MB (growth: +1.96 MB)

✅ Completed 1000 pool updates!

📊 MEMORY ANALYSIS
══════════════════

📈 Memory Statistics:
   • Initial memory:  245.32 MB
   • Final memory:    247.28 MB
   • Total growth:    +1.96 MB
   • Growth per 1k:   +1.96 MB
   • Growth percent:  +0.8%

📉 Memory Trend Analysis:
   • Slope: +0.0020 MB per 1k updates
   • Predicted at 10k:  265.12 MB (+19.80 MB growth)
   • Predicted at 100k: 445.32 MB (+200.00 MB growth)

✅ Memory growth is ACCEPTABLE
   • Small linear growth detected
   • Likely due to graph size increase

📊 Memory Usage Over Time:
   100 updates: [░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 246.15 MB
   200 updates: [█████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 246.42 MB
   300 updates: [██████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 246.58 MB
   400 updates: [██████████████░░░░░░░░░░░░░░░░░░░░░░░░░░] 246.71 MB
   500 updates: [███████████████████░░░░░░░░░░░░░░░░░░░░░] 246.83 MB
   600 updates: [███████████████████████░░░░░░░░░░░░░░░░░] 246.94 MB
   700 updates: [███████████████████████████░░░░░░░░░░░░░] 247.04 MB
   800 updates: [██████████████████████████████░░░░░░░░░░] 247.13 MB
   900 updates: [█████████████████████████████████░░░░░░░] 247.21 MB
  1000 updates: [████████████████████████████████████████] 247.28 MB

🧪 Validation
=============

✅ PASS: Memory growth within limit
   Growth 1.96 MB < 50.0 MB limit

✅ PASS: No significant memory leak detected
   Growth 1.96 MB is reasonable for 1000 updates

💡 Recommendations:
   ✅ Excellent memory management
   • Graph efficiently reuses allocations
   • Detector buffers are working correctly
```

## Test Configuration

### Memory Thresholds

```rust
const TOTAL_UPDATES: usize = 1000;           // Number of pool updates to simulate
const CHECK_INTERVAL: usize = 100;            // Check memory every N updates
const MAX_MEMORY_GROWTH_MB: f64 = 50.0;      // Max allowed growth over all updates
```

### Test Pool Data

The test uses 8 pool pairs across 3 tokens (SOL, USDC, USDT):
- 3 SOL/USDC pools (Raydium, Orca, Meteora)
- 2 USDC/USDT pools (Raydium, Orca)
- 3 reverse pairs for triangular arbitrage

Each update varies rates slightly to simulate real market movement.

## Validation Criteria

### ✅ Test Passes If:

1. **Memory growth < 50 MB** over 1,000 updates
2. **Total growth < 100 MB** (strict leak detection)
3. **Linear trend slope < 0.1 MB per 1k updates**

### ❌ Test Fails If:

1. Memory growth exceeds 50 MB limit
2. Exponential memory growth detected
3. Memory usage shows unbounded growth pattern

## Memory Analysis Features

### 1. Real-Time Monitoring
- Tracks physical memory usage via `memory-stats` crate
- Samples every 100 updates for overhead minimization
- Calculates growth from baseline

### 2. Trend Analysis
- Linear regression on memory samples
- Predicts memory at 10k and 100k updates
- Classifies growth as: Stable, Acceptable, or Concerning

### 3. Visual Representation
- ASCII bar chart showing memory over time
- Clear visualization of growth pattern
- Easy identification of memory spikes

### 4. Recommendations Engine
- Excellent (<10 MB growth): No action needed
- Good (10-50 MB growth): Monitor in production
- Concerning (>50 MB growth): Investigation required

## Troubleshooting

### Issue 1: "Unable to measure memory on this platform"

**Cause:** `memory-stats` doesn't support your OS  
**Solution:** Test will skip validation but still run logic

### Issue 2: Memory growth exceeds limit

**Possible Causes:**
1. Graph not properly clearing between updates
2. Detector buffers not being reused
3. HashMap allocations not released
4. Edge data accumulating

**Debug Steps:**
```bash
# Profile with flamegraph
cargo flamegraph --test integration_tests -- test_memory_usage_stable --ignored

# Check for leaks with valgrind (Linux only)
valgrind --leak-check=full --show-leak-kinds=all cargo test --test integration_tests test_memory_usage_stable -- --ignored

# Use heaptrack (Linux)
heaptrack cargo test --test integration_tests test_memory_usage_stable -- --ignored
```

### Issue 3: Test takes too long

**Cause:** 1000 updates might be slow on some systems  
**Solution:** Reduce `TOTAL_UPDATES` to 500 or 100 for quick checks

## Integration with CI/CD

### Add to GitHub Actions

```yaml
- name: Memory Stability Test
  run: cargo test --test integration_tests test_memory_usage_stable -- --ignored --nocapture
  timeout-minutes: 5
```

### Performance Baseline

Expected results on typical hardware:
- **Execution time:** 10-15 seconds
- **Memory growth:** 1-5 MB (excellent)
- **Memory growth:** 5-20 MB (acceptable)
- **Memory growth:** >50 MB (investigate)

## Advanced Usage

### Test with Optimized Bellman-Ford

After applying optimizations from `BELLMAN_FORD_OPTIMIZATIONS.md`:

```bash
# Expected improvement:
# - Before: ~10-20 MB growth
# - After:  ~1-5 MB growth (reusable buffers eliminate allocations)
```

### Stress Test (10k Updates)

Modify constants for long-running test:
```rust
const TOTAL_UPDATES: usize = 10_000;
const CHECK_INTERVAL: usize = 1000;
const MAX_MEMORY_GROWTH_MB: f64 = 200.0;
```

### Custom Pool Count

Increase pool count to test larger graphs:
```rust
fn create_test_pool_data_for_memory_test() -> Vec<...> {
    vec![
        // Add 50+ pool pairs to simulate mainnet DEX graph
    ]
}
```

## Metrics Tracked

| Metric | Description | Threshold |
|--------|-------------|-----------|
| Initial Memory | Baseline before updates | N/A |
| Final Memory | Memory after all updates | N/A |
| Total Growth | Final - Initial | <50 MB |
| Growth per 1k | Normalized growth rate | <5 MB |
| Growth Percent | Relative to baseline | <20% |
| Trend Slope | Linear regression slope | <0.1 MB/1k |
| Predicted 10k | Extrapolated memory | <500 MB |
| Predicted 100k | Long-term projection | N/A |

## Code Architecture

### Test Flow

```
1. Setup
   ├─ Create RPC client
   ├─ Initialize graph
   └─ Create detector

2. Baseline Measurement
   └─ Get initial memory usage

3. Simulation Loop (1000 iterations)
   ├─ Clear graph
   ├─ Add 8 pool pairs (16 edges)
   ├─ Run Bellman-Ford detection
   └─ Sample memory every 100 updates

4. Analysis
   ├─ Calculate statistics
   ├─ Linear regression
   ├─ Trend classification
   └─ Generate recommendations

5. Validation
   ├─ Assert growth < limit
   └─ Report pass/fail
```

### Helper Functions

**`get_memory_usage()`** - Returns current physical memory in MB  
**`create_test_pool_data_for_memory_test()`** - Generates 8 pool pairs

## Best Practices

### 1. Run Before Each Release
Ensure memory stability before deploying to production

### 2. Establish Baseline
Record typical memory growth for your environment

### 3. Monitor Trends
Compare results across commits to detect regressions

### 4. Production Correlation
Validate test results match production memory behavior

### 5. Regular Profiling
Use flamegraph quarterly to verify no new leaks introduced

## Related Tests

- `bench_arbitrage_detection_latency` - Performance benchmark
- `bench_end_to_end_latency` - Full pipeline benchmark
- `test_all_dex_combinations` - Functional validation

## Conclusion

The memory stability test ensures your MEV bot can run continuously without memory issues. With proper implementation (especially after applying Bellman-Ford optimizations), you should see:

✅ **<5 MB growth** per 1,000 updates  
✅ **Stable trend** over time  
✅ **No memory leaks** detected  
✅ **Production-ready** memory management  

This is critical for 24/7 MEV operations where memory leaks could cause crashes or performance degradation over time.
