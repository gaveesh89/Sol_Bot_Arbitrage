# Integration Tests - Quick Reference

## ✅ What's Been Created

### Test Files
- **`tests/integration_tests.rs`** (800+ lines) - 9 comprehensive integration tests
- **`tests/mainnet_fork_tests.rs`** (440+ lines) - Original mainnet fork tests  
- **`tests/helpers/mod.rs`** (600+ lines) - Test utilities and helpers

### Scripts
- **`setup-integration-tests.sh`** - Automated environment setup
- **`run-integration-tests.sh`** - Run tests with proper cleanup
- **`run-mainnet-fork-tests.sh`** - Legacy test runner

### Documentation
- **`INTEGRATION_TESTS_GUIDE.md`** (1000+ lines) - Complete testing guide
- **`MAINNET_FORK_QUICKSTART.md`** - Quick start guide
- **`MAINNET_FORK_IMPLEMENTATION.md`** - Technical details

## 🚀 Quick Start (30 seconds)

```bash
# 1. Install protobuf (one-time)
brew install protobuf  # macOS

# 2. Set API key
export HELIUS_API_KEY="your_key_here"

# 3. Run setup
./setup-integration-tests.sh

# 4. Run tests
./run-integration-tests.sh
```

## 📊 Test Suite Overview

| # | Test Name | Purpose | API | Time |
|---|-----------|---------|-----|------|
| 1 | `test_mainnet_fork_basic_setup` | Validator setup | ❌ | 5s |
| 2 | `test_fetch_real_raydium_pool_from_mainnet` | Fetch Raydium pool | ✅ | 3s |
| 3 | `test_fetch_multiple_dex_pools` | Fetch 3 DEX pools | ✅ | 5s |
| 4 | `test_detect_arbitrage_with_real_pools` | Run Bellman-Ford | ✅ | 8s |
| 5 | `test_build_and_validate_transaction` | Build & validate tx | ❌ | 10s |
| 6 | `test_execute_simulated_arbitrage_cycle` | Execute full cycle | ❌ | 15s |
| 7 | `test_profit_calculation_accuracy` | Verify profit math | ❌ | 1s |
| 8 | `test_transaction_size_limits` | Test size limits | ❌ | 2s |
| 9 | `test_compute_unit_estimation` | Estimate CU usage | ❌ | 1s |

**Total: 9 tests, ~50 seconds**

## 🎯 Key Features

✅ **Real Mainnet Data**
- Fetches actual pool states via Helius RPC
- Uses real Raydium, Orca, Meteora pool addresses
- Tests with current market conditions

✅ **Complete Validation**
- Transaction size ≤ 1232 bytes
- Compute units ≤ 1.4M CU
- Profit verification ±1% tolerance

✅ **Multi-DEX Support**
- Raydium AMM V4
- Orca Whirlpool
- Meteora DLMM

✅ **Production-Ready**
- Uses your actual bot code
- Comprehensive error handling
- Auto-cleanup of resources

## 📝 Common Commands

```bash
# Run all tests
./run-integration-tests.sh

# Run specific test
./run-integration-tests.sh test_fetch_real_raydium_pool_from_mainnet

# Run tests without API (offline)
cargo test --test integration_tests test_mainnet_fork_basic_setup -- --nocapture --ignored
cargo test --test integration_tests test_profit_calculation_accuracy -- --nocapture --ignored

# Debug with verbose output
RUST_LOG=debug ./run-integration-tests.sh test_name

# Clean up manually if needed
pkill -f solana-test-validator
rm -rf test-ledger/
```

## 🔧 Prerequisites

### Required
- ✅ Solana CLI tools (`solana-test-validator`)
- ✅ Protobuf compiler (`protoc`)
- ✅ Helius API key

### Optional
- Solscan API key (for enhanced pool discovery)

### Installation
```bash
# Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"

# Protobuf
brew install protobuf              # macOS
sudo apt-get install protobuf-compiler  # Linux

# Get API keys
# Helius: https://helius.dev
# Solscan: https://solscan.io
```

## 🏗️ Architecture

```
Integration Test Flow:
┌─────────────────────────────────────┐
│ Start solana-test-validator        │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│ Fetch real pool data (Helius API)  │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│ Build ArbitrageGraph                │
│ Run BellmanFordDetector             │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│ Build multi-hop transaction         │
│ Validate size & compute budget      │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│ Execute on forked mainnet           │
│ Verify profit                       │
└─────────────────────────────────────┘
```

## 🐛 Troubleshooting

### Validator Won't Start
```bash
pkill -f solana-test-validator
lsof -i :8899  # Check port
```

### API Key Issues
```bash
echo $HELIUS_API_KEY  # Verify set
export HELIUS_API_KEY="your_key"
```

### Build Fails
```bash
# Install protobuf
brew install protobuf

# Clean and rebuild
cargo clean
cargo build --tests
```

### Tests Hang
```bash
# Check validator logs
tail -f test-ledger/validator.log

# Kill and retry
pkill -f solana-test-validator
./run-integration-tests.sh
```

## 📈 Success Metrics

### Expected Results
- ✅ All 9 tests pass
- ✅ No validator errors
- ✅ Successful API calls
- ✅ Valid transaction construction
- ✅ Profit calculations accurate

### Performance Targets
- Test suite completes in <60 seconds
- Validator starts in <5 seconds
- API calls respond in <1 second
- Transaction size <1232 bytes
- Compute units <1.4M CU

## 🔗 Real Mainnet Addresses

**DEX Programs:**
```
Raydium AMM V4:  675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8
Orca Whirlpool:  whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc
Meteora DLMM:    LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo
```

**Test Pools:**
```
Raydium SOL/USDC:     58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2
Orca SOL/USDC:        7qbRF6YsyGuLUVs6Y1q64bdVrfe4ZcUUz1JRdoVNUJnm
Meteora SOL/USDC:     Bx7DRVY7zF8W6gZoVRgj3h6pKXK5RJBCovW6JkDz9X8z
```

## 📚 Documentation

- **INTEGRATION_TESTS_GUIDE.md** - Complete guide (read this first!)
- **MAINNET_FORK_QUICKSTART.md** - Quick reference
- **MAINNET_FORK_IMPLEMENTATION.md** - Technical deep dive
- **Test files** - Inline documentation in code

## 🎓 Next Steps

1. **Verify setup:**
   ```bash
   ./setup-integration-tests.sh
   ```

2. **Run basic test:**
   ```bash
   ./run-integration-tests.sh test_mainnet_fork_basic_setup
   ```

3. **Test API integration:**
   ```bash
   ./run-integration-tests.sh test_fetch_real_raydium_pool_from_mainnet
   ```

4. **Run full suite:**
   ```bash
   ./run-integration-tests.sh
   ```

5. **Read full guide:**
   ```bash
   cat INTEGRATION_TESTS_GUIDE.md
   ```

---

**Status:** ✅ Ready to Test  
**Created:** November 15, 2025  
**Version:** 1.0.0
