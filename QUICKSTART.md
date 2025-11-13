# Quick Start Guide

## Prerequisites

- Rust 1.70+ installed
- A Solana wallet with SOL for transaction fees
- RPC endpoint (use public or get a premium one from Helius/QuickNode)

## Installation

1. **Clone and setup:**
   ```bash
   cd /Users/gaveeshjain/Documents/VScode/Solana/BOT
   ./setup.sh
   ```

2. **Configure environment:**
   ```bash
   # Edit .env file with your settings
   nano .env
   ```

   Key settings to configure:
   - `RPC_URL`: Your Solana RPC endpoint
   - `WALLET_KEYPAIR_PATH`: Path to your wallet
   - `MIN_PROFIT_BPS`: Minimum profit threshold (50 = 0.5%)
   - Token mints and DEX program IDs

3. **Create or import wallet:**
   ```bash
   # Option 1: Generate new wallet
   solana-keygen new -o wallet.json

   # Option 2: Copy existing wallet
   cp ~/path/to/your/wallet.json ./wallet.json
   ```

4. **Build the project:**
   ```bash
   cargo build --release
   ```

## Running the Bot

### Development Mode (Recommended First)

Test on devnet before mainnet:

```bash
# Set devnet RPC in .env
RPC_URL=https://api.devnet.solana.com

# Get devnet SOL
solana airdrop 2 --url devnet

# Run bot
RUST_LOG=debug cargo run
```

### Production Mode

⚠️ **Use at your own risk!**

```bash
cargo run --release
```

## Configuration Overview

### Essential Settings

| Setting | Description | Example |
|---------|-------------|---------|
| `RPC_URL` | Solana RPC endpoint | `https://api.mainnet-beta.solana.com` |
| `WALLET_KEYPAIR_PATH` | Path to wallet file | `./wallet.json` |
| `MIN_PROFIT_BPS` | Min profit (basis points) | `50` (0.5%) |
| `MAX_SLIPPAGE_BPS` | Max slippage allowed | `100` (1%) |
| `CACHE_TTL_SECONDS` | Cache duration | `60` |
| `PRICE_CHECK_INTERVAL_MS` | Check frequency | `1000` (1 second) |

### DEX Program IDs (Mainnet)

These are pre-configured in `.env.example`:
- Raydium: `675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8`
- Meteora DLMM: `LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo`
- Whirlpool: `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc`
- Orca: `9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP`

## Adding Pools to Monitor

Edit `src/main.rs` and add pool addresses:

```rust
let pools_to_monitor = vec![
    (Pubkey::from_str("POOL_ADDRESS_1")?, DexType::Raydium),
    (Pubkey::from_str("POOL_ADDRESS_2")?, DexType::Meteora),
    (Pubkey::from_str("POOL_ADDRESS_3")?, DexType::Whirlpool),
];
```

### Finding Pool Addresses

1. **Raydium**: Visit [https://raydium.io/pools/](https://raydium.io/pools/)
2. **Meteora**: Visit [https://app.meteora.ag/pools](https://app.meteora.ag/pools)
3. **Orca**: Visit [https://www.orca.so/pools](https://www.orca.so/pools)
4. **Use Solana Explorer**: [https://explorer.solana.com/](https://explorer.solana.com/)

## Understanding the Output

When running, you'll see:

```
🚀 Starting Solana MEV Bot...
✅ Configuration loaded successfully
✅ RPC client initialized: https://api.mainnet-beta.solana.com
✅ Wallet loaded: YOUR_WALLET_ADDRESS
💰 Wallet balance: 0.5 SOL
✅ Token fetcher initialized with caching
✅ Market data fetcher initialized
📊 Initializing pool data...
✅ Pool data initialized
🎯 Bot initialization complete!
📈 Starting price monitoring...
```

When opportunities are found:

```
Found arbitrage opportunity: Buy on Raydium at 0.000123, Sell on Meteora at 0.000125, Profit: 162 bps
Opportunity: Token1 -> Token2 | Buy: Raydium @ 0.000123 | Sell: Meteora @ 0.000125 | Profit: 162 bps
```

## Safety Features

The bot includes several safety mechanisms:

1. **Profit Threshold**: Only executes if profit > `MIN_PROFIT_BPS`
2. **Slippage Protection**: Rejects trades exceeding `MAX_SLIPPAGE_BPS`
3. **Transaction Simulation**: Tests transactions before sending
4. **Balance Checks**: Warns if wallet balance is low
5. **Retry Logic**: Automatic retries with exponential backoff

## Monitoring Performance

### Check Logs

```bash
# View real-time logs
tail -f logs/mev_bot.log

# Filter for opportunities
grep "arbitrage opportunity" logs/mev_bot.log

# Check for errors
grep "ERROR" logs/mev_bot.log
```

### Important Metrics

- **Opportunities Found**: How many arb opportunities detected
- **Transactions Executed**: Successful arbitrage trades
- **Success Rate**: % of executed trades that were profitable
- **Average Profit**: Mean profit per successful trade

## Troubleshooting

### Bot doesn't start

1. Check `.env` configuration
2. Verify wallet file exists
3. Ensure RPC endpoint is accessible
4. Check Rust version: `rustc --version`

### No opportunities found

1. Verify pool addresses are correct
2. Check `MIN_PROFIT_BPS` isn't too high
3. Ensure sufficient market volatility
4. Try monitoring more pools

### Transactions fail

1. Check wallet has sufficient SOL
2. Verify compute budget settings
3. Increase `MAX_SLIPPAGE_BPS` (carefully!)
4. Use faster RPC endpoint

### High RPC costs

1. Enable caching (already default)
2. Increase `CACHE_TTL_SECONDS`
3. Increase `PRICE_CHECK_INTERVAL_MS`
4. Use batch fetching (already implemented)

## Performance Tuning

### For High Frequency

```env
PRICE_CHECK_INTERVAL_MS=100    # Check every 100ms
CACHE_TTL_SECONDS=5            # Short cache for fresh data
COMPUTE_UNIT_PRICE=5000        # Higher priority
```

### For Low Frequency / Cost Savings

```env
PRICE_CHECK_INTERVAL_MS=5000   # Check every 5 seconds
CACHE_TTL_SECONDS=300          # Long cache
COMPUTE_UNIT_PRICE=1000        # Standard priority
```

## Stopping the Bot

Press `Ctrl+C` to gracefully shutdown:

```
^C👋 Shutting down...
```

## Next Steps

1. **Test thoroughly on devnet** before going to mainnet
2. **Start with small amounts** for initial mainnet testing
3. **Monitor for 24 hours** before increasing size
4. **Implement additional DEX parsers** (see DEVELOPMENT.md)
5. **Add custom arbitrage strategies**
6. **Set up monitoring and alerts**

## Getting Help

- Check logs in `logs/mev_bot.log`
- Review `DEVELOPMENT.md` for technical details
- Ensure you understand MEV and arbitrage risks

## Important Warnings

⚠️ **Financial Risk**: MEV bots involve real money. You can lose funds.

⚠️ **Market Risk**: Prices change rapidly. Slippage can exceed expectations.

⚠️ **Technical Risk**: Bugs, RPC failures, and network issues can cause losses.

⚠️ **Regulatory Risk**: Ensure MEV trading is legal in your jurisdiction.

**Start small and test extensively!**
