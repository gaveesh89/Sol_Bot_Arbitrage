# 🔒 Safe Local Testing Guide

## Zero-Risk Local Testing with Helius RPC Clone

This guide explains how to test your MEV bot for 2-3 hours with **absolutely zero financial risk**.

---

## 🎯 Quick Start (100% Safe)

```bash
# Set your Helius RPC URL (optional, improves performance)
export HELIUS_RPC_URL="https://mainnet.helius-rpc.com/?api-key=YOUR_KEY"

# Run 2-hour safe test
./run-safe-local-test.sh 2

# Or 3-hour test
./run-safe-local-test.sh 3
```

**Risk Level: ZERO** ✅

---

## 🛡️ Why This is 100% Safe

### 1. **Local Test Validator**
- Runs completely **isolated** on your machine
- **No connection** to Solana mainnet for transactions
- Uses `solana-test-validator` (official Solana testing tool)
- Endpoint: `http://localhost:8899` (local only)

### 2. **Fake SOL Only**
- Airdrops **100 fake SOL** to test wallet
- This SOL has **zero real-world value**
- Cannot be transferred to mainnet
- Exists only in your local validator

### 3. **Cloned State (Read-Only)**
- **Reads** mainnet data from Helius (pools, programs)
- **Never writes** to mainnet
- Clone is snapshot - doesn't affect real chain
- One-way data flow: Mainnet → Local

### 4. **Dry-Run Mode**
- Transactions are **simulated**, not sent
- Bot validates logic without executing
- `dry_run_only = true` in config
- No actual on-chain state changes

### 5. **Manual Control**
- Press `Ctrl+C` to stop anytime
- No automated mainnet connections
- Wallet keypair stays local
- You control everything

---

## 📊 What Gets Tested

### ✅ Validated Functionality

1. **Arbitrage Detection**
   - Bellman-Ford algorithm correctness
   - Opportunity identification
   - Profitability calculations

2. **Transaction Building**
   - DEX instruction formatting
   - Multi-hop swap construction
   - Compute budget calculations

3. **Pool Data Parsing**
   - Raydium pool deserialization
   - Orca Whirlpool parsing
   - Meteora DLMM handling

4. **Performance Metrics**
   - Detection latency (<100ms)
   - Memory stability (no leaks)
   - CPU usage patterns

5. **Circuit Breaker**
   - Failure threshold detection
   - Alert triggering
   - Recovery mechanisms

6. **Monitoring**
   - Prometheus metrics collection
   - Counter increments
   - Histogram tracking

### ❌ What's NOT Tested

- **Real transaction execution** (requires mainnet)
- **Actual profit generation** (simulated only)
- **Network latency** to mainnet RPCs
- **MEV competition** with other bots
- **Jito bundle submission** (local validator doesn't support)

---

## 🚀 Step-by-Step Process

### Phase 1: Setup (Automatic)

The script automatically:

1. **Checks prerequisites**
   - Solana CLI installed
   - `solana-test-validator` available
   - Helius RPC URL (optional)

2. **Starts local validator**
   ```bash
   solana-test-validator \
       --url "https://mainnet.helius-rpc.com/..." \
       --clone <RAYDIUM_PROGRAM> \
       --clone <ORCA_PROGRAM> \
       --clone <METEORA_PROGRAM> \
       --ledger ./test-ledger \
       --rpc-port 8899
   ```

3. **Creates test wallet**
   ```bash
   solana-keygen new --outfile test-wallet.json
   ```

4. **Airdrops fake SOL**
   ```bash
   solana airdrop 100 <wallet_address>
   # This is FAKE SOL - zero value
   ```

### Phase 2: Configuration (Automatic)

Creates safe test config:

```toml
[rpc]
url = "http://localhost:8899"  # LOCAL ONLY

[wallet]
keypair_path = "./test-ledger/test-wallet.json"

[safety]
mode = "test"
allow_mainnet = false      # PREVENTS mainnet access
dry_run_only = true        # SIMULATES transactions
```

### Phase 3: Testing (2-3 Hours)

Runs bot in monitoring loop:

```
╔══════════════════════════════════════════════════════════════╗
║  🔒 SAFE LOCAL TEST - LIVE MONITORING                       ║
╚══════════════════════════════════════════════════════════════╝

Runtime:             1h 23m 45s
Opportunities:       47
Transactions:        42
Failed:              5
Success Rate:        89.4%

Logs:                ./logs/local-test-20251115-143022
Metrics Endpoint:    http://localhost:9090/metrics

Press Ctrl+C to stop test
```

### Phase 4: Cleanup (Automatic on Ctrl+C)

1. Stops bot gracefully
2. Stops local validator
3. Generates summary report
4. Preserves logs for analysis

---

## 📈 Monitoring During Test

### Live Metrics Dashboard

Open in browser while test runs:
```
http://localhost:9090/metrics
```

Example output:
```
# TYPE opportunities_detected counter
opportunities_detected 47

# TYPE transactions_sent counter
transactions_sent 42

# TYPE detection_latency_ms histogram
detection_latency_ms_count 47
detection_latency_ms_sum 1.567
```

### Command-Line Monitoring

```bash
# Watch metrics in real-time
watch -n 5 'curl -s http://localhost:9090/metrics | grep -E "(opportunities|transactions|latency)"'

# Check validator logs
tail -f ./logs/local-test-*/validator.log

# Check bot logs
tail -f ./logs/local-test-*/bot.log

# Check metrics history
tail -f ./logs/local-test-*/metrics.log
```

---

## 🔍 Sample Test Results

After 2-hour test, you'll see:

```
╔══════════════════════════════════════════════════════════════╗
║  📊 Test Summary Report                                      ║
╚══════════════════════════════════════════════════════════════╝

Final Metrics:
2025-11-15 16:30:45 | Runtime: 2h 0m 0s | Opportunities: 127 | Transactions: 114 | Failed: 13 | Success: 89.8%

Logs saved to:
  • Validator: ./logs/local-test-20251115-143022/validator.log
  • Bot:       ./logs/local-test-20251115-143022/bot.log  
  • Metrics:   ./logs/local-test-20251115-143022/metrics.log

✅ Test completed safely - no real funds used
```

### Interpreting Results

**Good Test:**
- Opportunities detected: 50-200 (depends on market)
- Success rate: >80%
- Detection latency: <100ms average
- No memory leaks (stable RAM usage)
- Circuit breaker: 0-2 triggers

**Issues to Investigate:**
- Success rate <50%: Logic errors in transaction building
- Detection latency >500ms: Performance optimization needed
- Circuit breaker >5 triggers: Too aggressive thresholds
- Memory growth: Memory leak in graph/detector

---

## 🔧 Troubleshooting

### Issue: Validator Won't Start

**Symptom:**
```
Error: Failed to start validator
```

**Solutions:**
1. Kill existing validator:
   ```bash
   pkill -f solana-test-validator
   ```

2. Clean ledger:
   ```bash
   rm -rf test-ledger
   ```

3. Check ports available:
   ```bash
   lsof -i :8899
   lsof -i :9900
   ```

### Issue: No Opportunities Detected

**Symptom:**
```
Opportunities: 0 (after 30+ minutes)
```

**Solutions:**
1. **Clone more accounts:**
   Edit script to add specific pool addresses:
   ```bash
   --clone <POOL_ADDRESS_1> \
   --clone <POOL_ADDRESS_2> \
   ```

2. **Check pool data:**
   ```bash
   solana account <POOL_ADDRESS> --url http://localhost:8899
   ```

3. **Lower profit threshold:**
   Edit config: `min_profit_threshold = 0.0001`

### Issue: High Failure Rate

**Symptom:**
```
Failed: 45 / 50 (90% failure rate)
```

**Solutions:**
1. **Check transaction building:**
   Review logs for error patterns

2. **Increase slippage:**
   Edit config: `slippage_tolerance = 0.05` (5%)

3. **Simplify routes:**
   Test with 2-hop only (A→B→A)

---

## 🎯 Advanced Testing Scenarios

### Test 1: Stress Test (High Volume)

Clone many pools for more opportunities:

```bash
# Edit run-safe-local-test.sh to add:
--clone <POOL_1> \
--clone <POOL_2> \
# ... (add 20-50 popular pools)
```

### Test 2: Memory Stability Test

Run for extended duration:

```bash
./run-safe-local-test.sh 8  # 8 hours
```

Monitor memory:
```bash
watch -n 30 'ps aux | grep mev-bot | grep -v grep'
```

### Test 3: Circuit Breaker Test

Intentionally trigger circuit breaker:

```toml
[circuit_breaker]
failure_threshold = 2      # Very low threshold
failure_window_secs = 30   # Short window
```

Verify:
- Alert is sent
- Bot halts operations
- Manual reset required

### Test 4: Performance Benchmark

```bash
# Run with performance profiling
cargo build --release --features metrics
perf record -F 99 -g ./target/release/mev-bot --config test-config.toml

# After test, analyze
perf report
```

---

## 📋 Pre-Test Checklist

Before running the 2-3 hour test:

- [ ] Solana CLI installed (`solana --version`)
- [ ] Test validator available (`solana-test-validator --version`)
- [ ] Helius API key set (optional): `export HELIUS_RPC_URL=...`
- [ ] Sufficient disk space (5+ GB for ledger)
- [ ] No other processes using ports 8899, 9900, 9090
- [ ] Bot compiles: `cargo build --release`
- [ ] Unit tests pass: `cargo test`

---

## 🚨 Safety Guarantees

### What CANNOT Happen

❌ **Cannot spend real SOL**
   - Test wallet has fake SOL only
   - Local validator isolated from mainnet

❌ **Cannot lose money**
   - No connection to real DEXs
   - No real token swaps executed

❌ **Cannot affect mainnet**
   - Read-only clone of state
   - Transactions stay local

❌ **Cannot drain wallet**
   - No real wallet exposed
   - Test keypair is disposable

### What CAN Happen

✅ **Use CPU/RAM**
   - Local validator + bot = moderate resource use
   - Can be stopped anytime

✅ **Fill disk**
   - Ledger grows over time (~1-2 GB/hour)
   - Auto-cleanup on script exit

✅ **Generate logs**
   - Debug logs can be large
   - Saved to `./logs/` directory

---

## 🎓 Learning Outcomes

After 2-3 hours of testing, you'll know:

1. **Does my bot detect arbitrage opportunities?**
   - Yes: Opportunities > 0
   - Quality: Success rate, profit calculations

2. **Is my transaction building correct?**
   - Yes: Low failure rate (<20%)
   - Issues: Check logs for errors

3. **How fast is detection?**
   - Target: <100ms for MEV competitiveness
   - Actual: Check latency histogram

4. **Is memory stable?**
   - Yes: Flat memory usage over time
   - Issue: Growing RAM = memory leak

5. **Does circuit breaker work?**
   - Yes: Triggers at threshold
   - Alert: Sent to console/webhook

6. **Am I ready for testnet?**
   - Yes: High success rate, stable memory, fast detection
   - No: Fix issues identified in test

---

## 🚀 Next Steps After Safe Test

### If Test Passes (>80% success rate)

1. **Deploy to Devnet**
   ```bash
   solana config set --url https://api.devnet.solana.com
   solana airdrop 2  # Free devnet SOL
   ./target/release/mev-bot --config devnet-config.toml
   ```

2. **Test with Small Mainnet Amount**
   - Fund wallet with 0.1 SOL (~$20)
   - Set `max_position_size = 0.01`
   - Monitor closely for 24 hours

3. **Gradual Scale-Up**
   - Week 1: 0.1 SOL max
   - Week 2: 0.5 SOL max  
   - Week 3: 1.0 SOL max
   - Monitor profitability

### If Test Fails (<50% success rate)

1. **Analyze logs**
   ```bash
   grep ERROR ./logs/local-test-*/bot.log
   ```

2. **Fix identified issues**
   - Transaction building errors
   - Pool parsing failures
   - Memory leaks

3. **Re-run safe test**
   - Verify fixes work
   - Repeat until passing

---

## 📚 FAQ

**Q: Can I lose money with this test?**  
A: **No.** Absolutely zero financial risk. All SOL is fake, all transactions are local.

**Q: How much does this test cost?**  
A: **$0.** Free to run. Optional Helius API key is free tier.

**Q: Can I run multiple tests in parallel?**  
A: No, each test uses same ports. Stop one before starting another.

**Q: What if I stop the test early (Ctrl+C)?**  
A: Safe! Cleanup runs automatically. Logs are preserved.

**Q: How do I test with real transactions?**  
A: After this passes, use Devnet (also free). Never mainnet initially.

**Q: Can I test Jito bundles locally?**  
A: No, local validator doesn't support Jito. Test on mainnet with tiny amounts.

**Q: What's a good success rate?**  
A: 70%+ is good, 85%+ is excellent. 100% is unrealistic (some opportunities expire).

**Q: How long should I test?**  
A: Minimum 2 hours. 3-8 hours for memory stability validation.

---

## ✅ Summary

**Safe Local Testing Provides:**
- ✅ Zero financial risk (fake SOL only)
- ✅ Realistic mainnet state (Helius clone)
- ✅ Performance validation (latency, memory)
- ✅ Circuit breaker testing
- ✅ Metrics collection
- ✅ Log analysis

**Perfect For:**
- Initial development validation
- Performance benchmarking
- Memory leak detection
- Circuit breaker verification
- Pre-devnet sanity checks

**Not a Replacement For:**
- Devnet testing (real network, real transactions)
- Mainnet testing (real profit, real competition)
- Long-term monitoring (days/weeks)

🎯 **Goal:** Validate your bot works correctly before risking any real funds!

**Ready to start?**
```bash
./run-safe-local-test.sh 2
```

🔒 **100% Safe | Zero Risk | Full Control**
