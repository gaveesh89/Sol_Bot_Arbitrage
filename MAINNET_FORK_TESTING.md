# Mainnet Fork Testing Guide

## Overview

This guide explains how to test the MEV bot against a **local Mainnet fork** using `solana-test-validator`. This approach provides:

- ✅ **Realistic Mainnet state** - Fork from live Mainnet with real pools and liquidity
- ✅ **Zero financial risk** - Test with unlimited fake SOL on local fork
- ✅ **Full transaction execution** - Run `execute_arbitrage_live` without spending real money
- ✅ **Profit validation** - Verify actual profit realization by comparing token balances
- ✅ **Fast iteration** - Test and debug quickly without network fees or delays

## Prerequisites

1. **Solana CLI Tools** installed (includes `solana-test-validator`)
2. **High-performance RPC endpoint** (Helius, QuickNode, or similar)
3. **Burner wallet keypair** (DO NOT use your main wallet)
4. **Configured .env file** with pool addresses

## Architecture

```
┌─────────────────────┐
│   Local Fork        │
│  (solana-test-      │
│   validator)        │
│  Port: 8899         │
│                     │
│  Forks from:        │
│  Mainnet RPC ───────┼──> Helius/QuickNode
│                     │
│  State: Mainnet     │
│  Funds: Unlimited   │
└──────────┬──────────┘
           │
           │ RPC Connection
           │ http://127.0.0.1:8899
           │
      ┌────▼─────┐
      │ MEV Bot  │
      │ (LIVE    │
      │  Mode)   │
      └──────────┘
```

## Execution Flow

### Quick Start (Automated Scripts)

**The easiest way to start:**

```bash
# Terminal 1 - Start the fork (automated)
export MAINNET_RPC_URL='https://mainnet.helius-rpc.com/?api-key=YOUR_KEY'
./start-mainnet-fork.sh

# Terminal 2 - Run the bot (automated)
./run-with-fork.sh
```

That's it! The scripts handle all the setup automatically.

---

### Manual Setup (Step-by-Step)

If you prefer manual control or the scripts don't work for your setup:

### Phase A: Start the Local Fork

**Terminal 1 - Fork Setup**

```bash
# Start the local validator, forking from a high-performance Mainnet RPC
# NOTE: Replace <YOUR_HIGH_PERFORMANCE_RPC> with your actual Helius/QuickNode URL
# Example: https://mainnet.helius-rpc.com/?api-key=YOUR_KEY

solana-test-validator \
  --url <YOUR_HIGH_PERFORMANCE_RPC> \
  --ledger /tmp/mainnet-fork-ledger \
  --reset \
  --rpc-port 8899 \
  --bind-address 0.0.0.0

# Expected output:
# Ledger location: /tmp/mainnet-fork-ledger
# Log: /tmp/mainnet-fork-ledger/validator.log
# Identity: <validator-pubkey>
# Genesis Hash: <hash>
# Version: <version>
# Shred Version: <version>
# Gossip Address: 127.0.0.1:1024
# TPU Address: 127.0.0.1:1027
# JSON RPC URL: http://127.0.0.1:8899
# WebSocket PubSub URL: ws://127.0.0.1:8900
```

**Command Breakdown:**
- `--url <YOUR_HIGH_PERFORMANCE_RPC>` - Fork from live Mainnet state
- `--ledger /tmp/mainnet-fork-ledger` - Store fork data locally
- `--reset` - Clean start (remove previous fork data)
- `--rpc-port 8899` - Standard RPC port
- `--bind-address 0.0.0.0` - Accept connections from all interfaces

**Important Notes:**
- ⚠️ Keep this terminal running - closing it stops the fork
- 💡 Initial sync may take 30-60 seconds
- 📊 Monitor the log: `tail -f /tmp/mainnet-fork-ledger/validator.log`

---

### Phase B: Fund the Test Wallet

**Terminal 2 - Wallet Setup**

```bash
# Airdrop test SOL to your burner wallet on the local fork
# NOTE: Replace <PATH_TO_BURNER_KEYPAIR> with the path to your keypair file
# Example: ./devnet-wallet.json or ~/.config/solana/burner-wallet.json

solana airdrop 100 \
  --url http://127.0.0.1:8899 \
  --keypair <PATH_TO_BURNER_KEYPAIR>

# Expected output:
# Requesting airdrop of 100 SOL
# Signature: <signature>
# 100 SOL

# Verify the balance
solana balance \
  --url http://127.0.0.1:8899 \
  --keypair <PATH_TO_BURNER_KEYPAIR>

# Expected output:
# 100 SOL
```

**Command Breakdown:**
- `solana airdrop 100` - Request 100 SOL (unlimited on local fork)
- `--url http://127.0.0.1:8899` - Target the local fork
- `--keypair <PATH>` - Specify the wallet to fund

**Important Notes:**
- ✅ You can airdrop unlimited SOL on the local fork
- 💰 Recommended: Start with 100 SOL for testing
- 🔑 Never use your real Mainnet wallet for testing

---

### Phase C: Run the Bot

**Terminal 3 - Bot Execution**

```bash
# Configure the bot to use the local fork and run in LIVE mode
# NOTE: This overrides RPC_URL and BOT_SIMULATION_MODE for this session only

LOCAL_FORK_URL="http://127.0.0.1:8899" \
BOT_SIMULATION_MODE=false \
cargo run --release

# Alternative: If LOCAL_FORK_URL doesn't work, use RPC_URL
RPC_URL="http://127.0.0.1:8899" \
BOT_SIMULATION_MODE=false \
cargo run --release

# Expected output (condensed):
# 🚀 Starting Solana MEV Bot...
# ✅ Configuration loaded successfully
# ✅ Wallet loaded: <your-wallet-pubkey>
# ✅ RPC client initialized
#    Primary RPC: http://127.0.0.1:8899  <-- Confirms fork connection
# 💰 Wallet balance: 100.0000 SOL
# 🎯 Bot initialization complete!
# ⚙️  Active Configuration Summary:
#    ├─ Execution Mode: LIVE 🔥  <-- Confirms live execution
# 🔄 Monitoring loop starting...
```

**Command Breakdown:**
- `LOCAL_FORK_URL="http://127.0.0.1:8899"` - Override RPC to use local fork (highest priority)
- `BOT_SIMULATION_MODE=false` - Enable LIVE transaction execution
- `cargo run --release` - Run the optimized release build

**Important Notes:**
- ⚠️ **LIVE MODE** - Bot will execute real transactions on the fork
- 🔍 Watch for: "Primary RPC: http://127.0.0.1:8899" to confirm fork connection
- 📊 Monitor logs for transaction signatures and profit validation
- 🛑 Press Ctrl+C to stop the bot

---

## Verification Checklist

Before running the bot, verify:

- [ ] **Terminal 1**: Local validator is running and synced
- [ ] **Terminal 2**: Test wallet is funded (100 SOL)
- [ ] **Terminal 3**: Bot logs show `Primary RPC: http://127.0.0.1:8899`
- [ ] **Terminal 3**: Bot logs show `Execution Mode: LIVE 🔥`
- [ ] **.env file**: Has valid pool addresses configured
- [ ] **Keypair**: Using burner wallet, NOT main wallet

## Expected Bot Behavior

When an arbitrage opportunity is found:

```
✅ Transaction confirmed!
   Signature: 5x7Kp2m...
   Explorer: https://explorer.solana.com/tx/5x7Kp2m...?cluster=custom&customUrl=http://127.0.0.1:8899
   Slot: 123456
   Compute units consumed: 45000
💰 Transaction executed successfully on-chain
   Block slot: 123456 (use for fork validation)
⚠️  TODO: Call validate_profit() here to verify profit realization
   Expected: Compare pre/post token balances for test wallet
   Location: Local fork at slot 123456
```

## Profit Validation (Manual)

Until `validate_profit()` is fully implemented, manually verify profits:

### Step 1: Check Token Balance Before Transaction

```bash
# Get your wallet's token accounts
solana account <YOUR_WALLET_PUBKEY> \
  --url http://127.0.0.1:8899 \
  --output json
```

### Step 2: Run Bot Transaction

Let the bot execute one arbitrage transaction.

### Step 3: Check Token Balance After Transaction

```bash
# Check the same token accounts
solana account <YOUR_WALLET_PUBKEY> \
  --url http://127.0.0.1:8899 \
  --output json

# Or use spl-token for specific tokens
spl-token balance <TOKEN_MINT> \
  --url http://127.0.0.1:8899 \
  --owner <YOUR_WALLET_PUBKEY>
```

### Step 4: Calculate Profit

Compare the before/after balances:
- **Input tokens** - Amount spent
- **Output tokens** - Amount received
- **SOL spent** - Transaction fees
- **Net profit** - (Output value) - (Input value) - (Fees)

## Troubleshooting

### Issue: "Connection refused" to localhost:8899

**Solution:** Local validator is not running. Start Phase A in Terminal 1.

```bash
# Check if validator is running
ps aux | grep solana-test-validator

# Check port availability
lsof -i :8899
```

---

### Issue: "Blockhash not found"

**Solution:** Local validator is still syncing. Wait 30-60 seconds after starting.

```bash
# Check validator logs
tail -f /tmp/mainnet-fork-ledger/validator.log
```

---

### Issue: Bot shows "Primary RPC: https://api.mainnet-beta.solana.com"

**Solution:** Environment variable not set correctly. Use `LOCAL_FORK_URL`:

```bash
# Correct
LOCAL_FORK_URL="http://127.0.0.1:8899" cargo run --release

# Also works
RPC_URL="http://127.0.0.1:8899" cargo run --release
```

---

### Issue: "Insufficient funds"

**Solution:** Airdrop more SOL to test wallet.

```bash
solana airdrop 100 \
  --url http://127.0.0.1:8899 \
  --keypair <PATH_TO_BURNER_KEYPAIR>
```

---

### Issue: "Pool account not found"

**Solution:** Pool doesn't exist on Mainnet or was recently created. Verify pool addresses:

```bash
# Check if pool exists on Mainnet
solana account <POOL_ADDRESS> \
  --url <YOUR_HIGH_PERFORMANCE_RPC>
```

---

### Issue: No arbitrage opportunities found

**Possible causes:**
1. **Pools have balanced prices** - Normal on Mainnet fork (snapshot in time)
2. **Low profit threshold** - Reduce `BOT_MIN_PROFIT_BPS` in .env
3. **Invalid pool configuration** - Check pool addresses in .env

**Solution:** Lower profit threshold temporarily:

```bash
LOCAL_FORK_URL="http://127.0.0.1:8899" \
BOT_SIMULATION_MODE=false \
BOT_MIN_PROFIT_BPS=10 \
cargo run --release
```

---

## Best Practices

### 1. Use High-Performance RPC

The fork quality depends on your RPC:
- ✅ **Recommended**: Helius, QuickNode, Triton
- ⚠️ **Avoid**: Free public RPCs (slow, rate-limited)

### 2. Reset Fork Between Tests

Clean state for consistent testing:

```bash
# Stop validator (Terminal 1: Ctrl+C)
# Remove old fork data
rm -rf /tmp/mainnet-fork-ledger

# Restart validator
solana-test-validator --url <YOUR_RPC> ...
```

### 3. Monitor System Resources

Local validator is resource-intensive:
- **RAM**: 8GB+ recommended
- **Disk**: SSD preferred
- **CPU**: 4+ cores recommended

```bash
# Monitor resource usage
top -pid $(pgrep solana-test-validator)
```

### 4. Test with Small Amounts First

Even on fork, test conservatively:
- Start with small trade sizes
- Verify profit calculation logic
- Gradually increase size after validation

### 5. Keep Fork Data for Debugging

Preserve fork state for post-mortem analysis:

```bash
# Copy fork data before reset
cp -r /tmp/mainnet-fork-ledger /tmp/mainnet-fork-backup-$(date +%Y%m%d)
```

---

## Advanced Configuration

### Automated Scripts

The repository includes helper scripts for easier fork testing:

#### start-mainnet-fork.sh

Automates the entire fork setup process:

```bash
# Basic usage (uses ./devnet-wallet.json, airdrops 100 SOL)
export MAINNET_RPC_URL='https://mainnet.helius-rpc.com/?api-key=YOUR_KEY'
./start-mainnet-fork.sh

# Custom keypair
./start-mainnet-fork.sh ./my-wallet.json

# Custom keypair and airdrop amount
./start-mainnet-fork.sh ./my-wallet.json 50
```

**Features:**
- ✅ Checks prerequisites (solana-test-validator, RPC URL)
- ✅ Cleans up old fork data automatically
- ✅ Starts validator in background
- ✅ Waits for validator to sync
- ✅ Airdrops SOL to test wallet
- ✅ Displays connection details and next steps
- ✅ Monitors validator logs (Ctrl+C to stop)

#### run-with-fork.sh

Runs the bot with correct environment variables:

```bash
# Basic usage (connects to http://127.0.0.1:8899)
./run-with-fork.sh

# Custom fork URL
./run-with-fork.sh http://127.0.0.1:8898
```

**Features:**
- ✅ Checks if fork is running
- ✅ Verifies wallet has funds
- ✅ Sets LOCAL_FORK_URL and BOT_SIMULATION_MODE
- ✅ Builds bot if needed
- ✅ Shows configuration before starting

---

### Custom Fork Port

Use different port if 8899 is occupied:

```bash
# Terminal 1 - Validator
solana-test-validator \
  --url <YOUR_RPC> \
  --rpc-port 8898 \
  ...

# Terminal 3 - Bot
LOCAL_FORK_URL="http://127.0.0.1:8898" \
cargo run --release
```

### Fork Specific Slot

Fork from a specific historical slot:

```bash
solana-test-validator \
  --url <YOUR_RPC> \
  --clone <POOL_ADDRESS> \
  --clone <TOKEN_MINT> \
  --rpc-port 8899
```

### Enable Transaction Logs

See detailed transaction logs in validator:

```bash
solana-test-validator \
  --url <YOUR_RPC> \
  --log \
  --rpc-port 8899
```

---

## Next Steps

After successful fork testing:

1. **Implement `validate_profit()` function** (see TODO in `src/chain/executor.rs`)
2. **Add automated balance tracking** (capture pre/post transaction state)
3. **Set up monitoring dashboards** (track profit/loss over multiple transactions)
4. **Optimize transaction parameters** (compute units, priority fees)
5. **Test with multiple pool configurations** (different DEXs, tokens)
6. **Prepare for Mainnet deployment** (security audit, monitoring setup)

---

## Security Reminder

⚠️ **CRITICAL**: This is for TESTING ONLY

- ✅ Use burner wallets with no real funds
- ✅ Test on local fork before any Mainnet deployment
- ✅ Never commit keypair files to git
- ✅ Keep Mainnet RPC URLs in environment variables, not code
- ❌ Do NOT run live mode on actual Mainnet until fully tested
- ❌ Do NOT use your main wallet for testing

---

## Resources

- [Solana Test Validator Docs](https://docs.solana.com/developing/test-validator)
- [Solana CLI Reference](https://docs.solana.com/cli)
- [MEV Bot Configuration Guide](./QUICKSTART.md)
- [Devnet Testing Guide](./DEVNET_TESTING.md)
- [Security Best Practices](./SECURITY.md)

---

## Summary

**Mainnet Fork Testing Flow:**

1. **Start Fork** → `solana-test-validator --url <MAINNET_RPC> ...`
2. **Fund Wallet** → `solana airdrop 100 --url http://127.0.0.1:8899 ...`
3. **Run Bot** → `LOCAL_FORK_URL="http://127.0.0.1:8899" BOT_SIMULATION_MODE=false cargo run --release`
4. **Verify Execution** → Check logs for "Primary RPC: http://127.0.0.1:8899" and "LIVE 🔥"
5. **Validate Profit** → Compare token balances before/after transactions
6. **Iterate** → Adjust parameters, reset fork, repeat

**Benefits:**
- 🎯 Test against real Mainnet state
- 💰 Zero financial risk
- 🚀 Fast iteration cycles
- ✅ Full transaction execution validation

Happy testing! 🚀
