# Devnet Testing Guide

## 🧪 Overview

This guide will help you test your Solana MEV arbitrage bot on **Devnet** - Solana's test network where tokens have no real value. This is the **safest way** to validate your bot before risking real funds on mainnet.

---

## 📋 Prerequisites

### 1. Install Solana CLI Tools
```bash
# Check if already installed
solana --version

# If not installed (macOS)
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"

# Add to PATH (if needed)
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"

# Verify installation
solana --version
# Should show: solana-cli 1.18.x or later
```

### 2. Configure Solana CLI for Devnet
```bash
# Set cluster to Devnet
solana config set --url devnet

# Verify configuration
solana config get
# Should show: RPC URL: https://api.devnet.solana.com
```

---

## 🔑 Step 1: Create Test Wallet

### Generate New Devnet Wallet
```bash
# Create a new keypair for Devnet testing
solana-keygen new --outfile ./devnet-wallet.json --no-bip39-passphrase

# IMPORTANT: This wallet is for TESTING ONLY
# Do NOT use your mainnet wallet on Devnet!

# Check the public key
solana-keygen pubkey ./devnet-wallet.json
```

**Output Example:**
```
Generating a new keypair

For added security, enter a BIP39 passphrase

NOTE! This passphrase improves security of the recovery seed phrase NOT the
keypair file itself, which is stored as insecure plain text

BIP39 Passphrase (empty for none): 

Wrote new keypair to ./devnet-wallet.json
================================================================================
pubkey: 7xXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxXxX
================================================================================
Save this seed phrase and your BIP39 passphrase to recover your new keypair:
word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
================================================================================
```

### Secure the Keypair File
```bash
# Set restrictive permissions (owner read/write only)
chmod 600 ./devnet-wallet.json

# Verify permissions
ls -la ./devnet-wallet.json
# Should show: -rw------- (600)
```

---

## 💰 Step 2: Fund Your Test Wallet

### Request Devnet SOL (Airdrop)
```bash
# Request 2 SOL from Devnet faucet
solana airdrop 2 --url devnet --keypair ./devnet-wallet.json

# Check balance
solana balance --url devnet --keypair ./devnet-wallet.json
# Should show: 2 SOL

# Request more if needed (max 5 SOL per request, rate limited)
solana airdrop 3 --url devnet --keypair ./devnet-wallet.json
solana balance --url devnet --keypair ./devnet-wallet.json
# Should show: 5 SOL
```

**Troubleshooting Airdrop Issues:**
```bash
# If airdrop fails due to rate limiting, try:
# 1. Wait 5-10 minutes and try again
# 2. Use alternative faucet websites:
#    - https://faucet.solana.com/
#    - https://solfaucet.com/

# Check your wallet's public key
solana-keygen pubkey ./devnet-wallet.json
# Then paste it into the web faucet
```

---

## ⚙️ Step 3: Configure Bot for Devnet

### Copy Devnet Environment File
```bash
# Copy the Devnet configuration template
cp .env.devnet .env

# Verify the configuration
cat .env | grep -E "RPC_URL|WALLET_KEYPAIR_PATH|BOT_SIMULATION_MODE"
```

**Expected Output:**
```
RPC_URL=https://api.devnet.solana.com
WALLET_KEYPAIR_PATH=./devnet-wallet.json
BOT_SIMULATION_MODE=true
```

### Verify Configuration
```bash
# Check that environment variables are set correctly
source .env
echo "RPC: $RPC_URL"
echo "Wallet: $WALLET_KEYPAIR_PATH"
echo "Simulation Mode: $BOT_SIMULATION_MODE"
```

---

## 🏗️ Step 4: Build the Bot

### Compile in Release Mode
```bash
# Build optimized binary
cargo build --release

# Check build status
echo "Build exit code: $?"
# Should show: 0 (success)
```

**Troubleshooting Build Errors:**
```bash
# If build fails, try:
cargo clean
cargo update
cargo build --release

# Check for specific errors
cargo check 2>&1 | grep error
```

---

## 🧪 Step 5: Test in Simulation Mode (Recommended First Step)

### Run Simulation Mode
```bash
# Set simulation mode (safest option)
export BOT_SIMULATION_MODE=true

# Run the bot
./target/release/mev-bot

# Or run with debug logging
RUST_LOG=debug ./target/release/mev-bot
```

**What to Look For:**
```
✅ Execution Mode: SIMULATION (Zero-risk testing)
✅ Wallet loaded successfully
✅ Balance sufficient (>0.5 SOL)
✅ Connected to Devnet RPC
✅ Monitoring DEX pools
```

**Example Output:**
```
2025-11-14T12:00:00Z INFO  mev_bot] Starting MEV Arbitrage Bot
2025-11-14T12:00:00Z INFO  mev_bot] ========================================
2025-11-14T12:00:00Z INFO  mev_bot] CONFIGURATION SUMMARY
2025-11-14T12:00:00Z INFO  mev_bot] ========================================
2025-11-14T12:00:00Z INFO  mev_bot] 🔹 Execution Mode: SIMULATION (Zero-risk testing)
2025-11-14T12:00:00Z INFO  mev_bot] 🔹 RPC URL: https://api.devnet.solana.com
2025-11-14T12:00:00Z INFO  mev_bot] 🔹 Wallet: 7xXx...XxXx
2025-11-14T12:00:00Z INFO  mev_bot] 🔹 Balance: 5.0 SOL
2025-11-14T12:00:00Z INFO  mev_bot] ========================================
```

### Monitor Simulation Results
```bash
# Run and watch for opportunities
./target/release/mev-bot 2>&1 | grep -E "SIMULATION|Opportunity|Profit"

# Expected to see:
# - Pool monitoring logs
# - Opportunity detection (if any exist on Devnet)
# - Simulation results with compute units
```

---

## 🚀 Step 6: Test Live Execution on Devnet

### Switch to Live Mode (After Simulation Success)
```bash
# ⚠️ WARNING: This will execute REAL transactions on Devnet
# Ensure you have sufficient Devnet SOL for transaction fees

# Set live execution mode
export BOT_SIMULATION_MODE=false

# Verify balance before running
solana balance --url devnet --keypair ./devnet-wallet.json
# Should have at least 1 SOL for fees

# Run the bot in live mode
./target/release/mev-bot
```

**What to Expect:**
```
⚠️  LIVE EXECUTION MODE ENABLED
⚠️  Real transactions will be submitted to Devnet
⚠️  Ensure you have tested in simulation mode first
```

### Monitor Live Transactions
```bash
# In another terminal, monitor your wallet
watch -n 5 "solana balance --url devnet --keypair ./devnet-wallet.json"

# View transaction history
solana transaction-history --url devnet $(solana-keygen pubkey ./devnet-wallet.json)
```

### Check Transaction on Solscan
```bash
# Get your wallet address
solana-keygen pubkey ./devnet-wallet.json

# View on Solscan Devnet Explorer:
# https://solscan.io/account/YOUR_WALLET_ADDRESS?cluster=devnet
```

---

## 📊 Step 7: Analyze Results

### Check Bot Performance
```bash
# View detailed logs
tail -f logs/mev-bot.log  # If logging to file

# Or use grep to filter important events
./target/release/mev-bot 2>&1 | grep -E "SUCCESS|FAILED|Profit|Units"
```

### Key Metrics to Monitor
1. **Simulation Success Rate**
   - How many opportunities pass simulation?
   - What's the typical compute unit usage?

2. **Execution Success Rate** (Live Mode)
   - How many transactions confirm?
   - What's the average confirmation time?

3. **Profitability**
   - Are detected opportunities actually profitable?
   - Do fees eat into profits?

4. **Error Analysis**
   - What types of errors occur most?
   - Are they network issues or logic problems?

---

## 🔍 Troubleshooting

### Issue: No Arbitrage Opportunities Found
**Cause:** Devnet has limited liquidity and fewer active DEX pools than mainnet.

**Solutions:**
```bash
# 1. Check if DEX programs are deployed on Devnet
solana program show --url devnet <PROGRAM_ID>

# 2. Use Mainnet-fork for testing instead
# See MAINNET_FORK_TESTING.md (advanced)

# 3. Create test pools yourself on Devnet (advanced)
```

### Issue: Airdrop Rate Limited
**Cause:** Devnet faucet limits requests to prevent abuse.

**Solutions:**
```bash
# 1. Wait 5-10 minutes between airdrop requests

# 2. Use web-based faucets:
# - https://faucet.solana.com/
# - https://solfaucet.com/

# 3. Request smaller amounts more frequently
solana airdrop 1 --url devnet --keypair ./devnet-wallet.json
```

### Issue: Simulation Fails with "Blockhash Not Found"
**Cause:** Transaction blockhash expired before simulation.

**Solutions:**
```bash
# 1. Increase RPC timeout in .env
RPC_TIMEOUT_SECONDS=60

# 2. Use a faster RPC endpoint
# Consider Helius or Alchemy for Devnet

# 3. Reduce delay between transaction creation and simulation
```

### Issue: Transaction Fails with "Insufficient Funds"
**Cause:** Not enough SOL for rent + fees.

**Solutions:**
```bash
# Check balance
solana balance --url devnet --keypair ./devnet-wallet.json

# Request more SOL
solana airdrop 5 --url devnet --keypair ./devnet-wallet.json

# Reduce position size in .env
MAX_POSITION_SIZE=50000000  # 0.05 SOL
```

### Issue: High Compute Unit Usage (>1.4M)
**Cause:** Transaction too complex or inefficient.

**Solutions:**
```bash
# 1. Optimize swap instructions
# 2. Reduce number of hops (MAX_HOPS=2)
# 3. Use direct routes only (PREFER_DIRECT_ROUTES=true)
# 4. Check logs for expensive operations
```

---

## 🎯 Testing Checklist

Before moving to mainnet, verify:

- [ ] ✅ Simulation mode works without errors
- [ ] ✅ Bot connects to Devnet RPC successfully
- [ ] ✅ Wallet loads and has sufficient balance
- [ ] ✅ Pool monitoring detects DEX pools (if available)
- [ ] ✅ Compute unit usage is under 1.4M limit
- [ ] ✅ Live transactions confirm successfully (if testing live mode)
- [ ] ✅ Error handling works correctly
- [ ] ✅ Logs provide useful debugging information
- [ ] ✅ Bot can run for extended periods without crashing
- [ ] ✅ Security checks pass (no exposed private keys, etc.)

---

## 🚦 Next Steps

### If Testing Successful ✅
1. Review and optimize based on Devnet results
2. Consider **Mainnet-fork testing** for realistic conditions (see advanced guide)
3. Start with **small position sizes** on mainnet
4. Keep **simulation mode enabled** initially on mainnet
5. Gradually increase position sizes as confidence grows

### If Issues Found ❌
1. Review error logs carefully
2. Test individual components in isolation
3. Verify DEX program deployments on Devnet
4. Consider using mainnet-fork for more realistic testing
5. Ask for help in Solana developer communities

---

## 📚 Additional Resources

### Solana Devnet
- **Devnet RPC:** https://api.devnet.solana.com
- **Devnet Explorer:** https://explorer.solana.com/?cluster=devnet
- **Devnet Faucet:** https://faucet.solana.com/
- **Devnet Status:** https://status.solana.com/

### Documentation
- **Solana Docs:** https://docs.solana.com/
- **Solana CLI:** https://docs.solana.com/cli
- **RPC API:** https://docs.solana.com/api

### DEX Programs on Devnet
- **Raydium:** Check if deployed on Devnet
- **Orca:** May have test pools on Devnet
- **Meteora:** Verify Devnet deployment

### Community
- **Solana Discord:** https://discord.gg/solana
- **Solana Stack Exchange:** https://solana.stackexchange.com/
- **GitHub Issues:** https://github.com/solana-labs/solana/issues

---

## ⚠️ Important Reminders

1. **Devnet tokens have NO real value** - Safe for testing!
2. **Never use mainnet wallet on Devnet** - Keep them separate
3. **Devnet can be reset** - Wallets and balances may be wiped
4. **Limited liquidity on Devnet** - Fewer arbitrage opportunities
5. **Network conditions differ** - Mainnet is faster and more congested

**Happy Testing! 🚀**
