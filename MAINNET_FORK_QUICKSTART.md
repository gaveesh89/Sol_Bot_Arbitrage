# Mainnet Fork Integration Tests - Quick Start

Complete setup guide for running Solana arbitrage tests on forked mainnet.

## 🚀 Quick Start (5 minutes)

### Step 1: Install Prerequisites

```bash
# Install Solana CLI tools (includes test-validator)
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"

# Verify installation
solana-test-validator --version
```

### Step 2: Get API Keys

1. **Helius API** (free tier available)
   - Sign up: https://helius.dev
   - Create project → Copy API key

2. **Solscan API** (free tier available)
   - Sign up: https://solscan.io
   - API section → Generate key

### Step 3: Set Environment Variables

```bash
# Option A: Export directly
export HELIUS_API_KEY="your_helius_key_here"
export SOLSCAN_API_KEY="your_solscan_key_here"

# Option B: Create .env file
cp .env.example .env
# Edit .env with your keys
```

### Step 4: Run Tests

```bash
# Run all tests
./run-mainnet-fork-tests.sh

# Run specific test
./run-mainnet-fork-tests.sh test_execute_swap_on_raydium

# Manual run with cargo
cargo test --test mainnet_fork_tests -- --test-threads=1 --nocapture --ignored
```

## 📋 Test Suite Overview

### Available Tests

| Test Name | Purpose | Duration |
|-----------|---------|----------|
| `test_fork_mainnet_and_fetch_pools` | Setup validation | ~5s |
| `test_fetch_raydium_pool_data` | Pool data fetching | ~3s |
| `test_fetch_orca_whirlpool_data` | Whirlpool data | ~3s |
| `test_detect_triangular_arbitrage_opportunity` | Profit calculation | ~5s |
| `test_execute_swap_on_raydium` | Single swap execution | ~10s |
| `test_execute_triangular_arbitrage` | Full arbitrage cycle | ~15s |
| `test_compute_budget_optimization` | CU optimization | ~20s |
| `test_transaction_size_optimization` | Size optimization | ~10s |
| `test_profit_verification` | Profit accuracy | ~15s |

### Test Categories

**🔧 Setup Tests** - Verify environment is configured correctly
- `test_fork_mainnet_and_fetch_pools`

**📊 Data Fetching Tests** - Test mainnet data retrieval
- `test_fetch_raydium_pool_data`
- `test_fetch_orca_whirlpool_data`

**💰 Arbitrage Tests** - Test opportunity detection and execution
- `test_detect_triangular_arbitrage_opportunity`
- `test_execute_triangular_arbitrage`
- `test_profit_verification`

**⚡ Optimization Tests** - Test performance constraints
- `test_compute_budget_optimization`
- `test_transaction_size_optimization`

## 🎯 Example Test Run

```bash
$ ./run-mainnet-fork-tests.sh test_execute_swap_on_raydium

═══════════════════════════════════════════════════════
  Solana Mainnet Fork Integration Testing Setup
═══════════════════════════════════════════════════════

[1/5] Checking for solana-test-validator...
✓ Found: solana-test-validator 1.18.0

[2/5] Checking for API keys...
✓ HELIUS_API_KEY: a1b2c3d4...
✓ SOLSCAN_API_KEY: x9y8z7w6...

[3/5] Cleaning up existing test validators...
✓ No validators running

[4/5] Building project...
✓ Build successful

[5/5] Running tests...
═══════════════════════════════════════════════════════
Running specific test: test_execute_swap_on_raydium

running 1 test
🔄 Executing swap on Raydium (forked mainnet)...
🚀 Starting test validator with mainnet fork...
🔑 API keys loaded
   Helius: a1b2c3d4...
   Solscan: x9y8z7w6...
✅ Validator ready after 5 attempts
✅ Funded wallet: 7xK9...m2Lp
✅ Built swap transaction: 450 bytes
✅ Swap executed successfully!
   Signature: 5Jx9...k2Lp
   Final balance: 8950000000 lamports
test test_execute_swap_on_raydium ... ok

test result: ok. 1 passed; 0 failed; 0 ignored

═══════════════════════════════════════════════════════
Cleaning up...
✓ Done!
```

## 📁 Project Structure

```
BOT/
├── tests/
│   ├── mainnet_fork_tests.rs      # Main test suite (500+ lines)
│   └── helpers/
│       └── mod.rs                  # Test utilities (600+ lines)
├── run-mainnet-fork-tests.sh      # Quickstart script
├── .env.example                    # Environment template
└── MAINNET_FORK_TESTING.md        # Full documentation
```

## ⚙️ Configuration

### Transaction Limits

```rust
const MAX_TRANSACTION_SIZE: usize = 1232;  // bytes
const MAX_COMPUTE_UNITS: u32 = 1_400_000;  // CU
```

### Known Program IDs

```rust
// DEX Programs
const RAYDIUM_AMM_V4: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const ORCA_WHIRLPOOL: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
const METEORA_DLMM: &str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";

// Popular Pools
const RAYDIUM_USDC_SOL: &str = "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2";
```

## 🐛 Troubleshooting

### Validator Won't Start

```bash
# Kill existing validators
pkill -f solana-test-validator

# Check port availability
lsof -i :8899

# Try with different port
TEST_VALIDATOR_PORT=8900 cargo test --test mainnet_fork_tests
```

### API Key Errors

```bash
# Verify keys are set
echo $HELIUS_API_KEY
echo $SOLSCAN_API_KEY

# Check .env file
cat .env

# Reload environment
source .env
```

### Build Errors

```bash
# Clean and rebuild
cargo clean
cargo build --tests

# Update dependencies
cargo update
```

### Test Failures

```bash
# Run with verbose output
cargo test --test mainnet_fork_tests -- --nocapture

# Run single test for debugging
cargo test test_fork_mainnet_and_fetch_pools -- --nocapture

# Check validator logs
tail -f test-ledger/validator.log
```

## 📊 Key Features

### ✅ Real Mainnet Data
- Fetches actual pool states via Helius RPC
- Uses real token addresses and program IDs
- Validates with current mainnet liquidity

### ✅ Accurate Profit Calculation
- Accounts for DEX fees (typically 0.25%)
- Includes slippage estimation
- Verifies actual vs expected profit (±1% tolerance)

### ✅ Transaction Validation
- Enforces 1232 byte size limit
- Respects 1.4M compute unit limit
- Tests multi-hop transaction construction

### ✅ DEX Integration
- Raydium AMM V4 swaps
- Orca Whirlpool concentrated liquidity
- Meteora DLMM support (coming soon)

## 🔗 Next Steps

1. **Run Basic Tests** - Verify setup works
   ```bash
   ./run-mainnet-fork-tests.sh test_fork_mainnet_and_fetch_pools
   ```

2. **Test Data Fetching** - Confirm API keys work
   ```bash
   ./run-mainnet-fork-tests.sh test_fetch_raydium_pool_data
   ```

3. **Execute Simple Swap** - Test transaction building
   ```bash
   ./run-mainnet-fork-tests.sh test_execute_swap_on_raydium
   ```

4. **Run Full Suite** - Comprehensive validation
   ```bash
   ./run-mainnet-fork-tests.sh
   ```

5. **Review Documentation** - Deep dive into details
   ```bash
   cat MAINNET_FORK_TESTING.md
   ```

## 📚 Additional Resources

- **Full Documentation**: `MAINNET_FORK_TESTING.md`
- **Test Code**: `tests/mainnet_fork_tests.rs`
- **Helper Utilities**: `tests/helpers/mod.rs`
- **Solana Docs**: https://docs.solana.com/
- **Helius Docs**: https://docs.helius.dev/
- **Raydium SDK**: https://github.com/raydium-io/raydium-sdk

## 💡 Tips

- Tests are marked with `#[ignore]` - remove or use `--ignored` flag
- Use `--nocapture` to see println! output
- Run with `--test-threads=1` for serial execution
- Check validator logs in `test-ledger/` directory
- Clean up with `pkill -f solana-test-validator` if stuck

## 🤝 Contributing

When adding new tests:
1. Use `#[serial]` attribute for sequential execution
2. Add `#[ignore]` to prevent accidental runs
3. Include descriptive println! messages
4. Handle cleanup in Drop implementation
5. Document expected behavior

## ⚠️ Important Notes

- **Never commit API keys** - They're in `.gitignore`
- **Tests fork mainnet state** - Requires internet connection
- **Validator runs locally** - No actual SOL spent
- **Rate limits apply** - Be mindful of API quotas
- **Serial execution required** - One test at a time

---

**Status**: Ready for testing ✅  
**Last Updated**: November 15, 2025  
**Solana Version**: 1.18+  
**Rust Version**: 1.75+
