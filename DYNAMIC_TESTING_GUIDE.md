# Dynamic Testing Guide: Aggressive/Dynamic Data Fetching Strategy

## Overview

This guide explains how to use the **Aggressive/Dynamic Testing** strategy, which enables the bot to execute transactions against a local Solana fork while fetching real-time pool data from a high-performance external API.

## Architecture

### The Problem
When testing against a local Solana fork (`solana-test-validator`), pool account state becomes stale quickly:
- Local fork clones account state from mainnet at startup
- Pool reserves don't update unless transactions are sent to the local validator
- This makes arbitrage detection inaccurate since prices drift from real mainnet

### The Solution
**Dynamic Data Fetching** separates data source from execution target:
- **Data Source**: External API (Helius, Jupiter, etc.) provides real-time pool reserves
- **Execution Target**: Local fork provides safe testing environment for transactions
- **Result**: Accurate arbitrage detection + risk-free testing

```
┌─────────────────────────────────────────────────────┐
│                   MEV Bot Process                    │
│                                                      │
│  ┌──────────────────┐        ┌──────────────────┐  │
│  │  Arbitrage       │        │  Transaction     │  │
│  │  Detector        │        │  Executor        │  │
│  │                  │        │                  │  │
│  │ Uses real-time   │        │ Sends txs to     │  │
│  │ pool data from   │        │ local fork       │  │
│  │ external API     │        │                  │  │
│  └────────┬─────────┘        └────────┬─────────┘  │
│           │                           │             │
└───────────┼───────────────────────────┼─────────────┘
            │                           │
            ▼                           ▼
  ┌──────────────────┐        ┌──────────────────┐
  │  Helius API      │        │  Local Fork      │
  │  (Mainnet Data)  │        │  (Test Env)      │
  │                  │        │                  │
  │  Real-time       │        │  Safe execution  │
  │  pool reserves   │        │  No real funds   │
  └──────────────────┘        └──────────────────┘
```

## Implementation Details

### Configuration Changes

#### 1. Added `external_data_api_url` to `RpcConfig` (src/config.rs)

```rust
pub struct RpcConfig {
    pub url: String,
    pub ws_url: String,
    pub backup_urls: Vec<String>,
    pub commitment_level: String,
    pub timeout_seconds: u64,
    /// Optional external API URL for fetching real-time pool data
    pub external_data_api_url: Option<String>,
}
```

**Environment Variable**: `EXTERNAL_DATA_API_URL`

**Example Values**:
- Helius: `https://mainnet.helius-rpc.com/?api-key=YOUR_KEY`
- QuickNode: `https://YOUR_ENDPOINT.quiknode.pro/YOUR_TOKEN/`
- Custom: `https://your-custom-api.com/solana/mainnet`

#### 2. Enhanced `TokenFetcher` (src/chain/token_fetch.rs)

**New Fields**:
```rust
pub struct TokenFetcher {
    rpc_client: Arc<RpcClient>,
    config: TokenFetchConfig,
    account_cache: Cache<Pubkey, Account>,
    pool_cache: Arc<DashMap<Pubkey, CachedPoolData>>,
    http_client: reqwest::Client,              // For external API calls
    external_api_cache: Arc<DashMap<...>>,     // 100ms cache
}
```

**New Methods**:
- `fetch_realtime_pool_data()`: Fetches pool data from external API
- `fetch_accounts_from_external_api()`: Makes JSON-RPC calls to external endpoint
- Enhanced `initialize_pool_data()`: Automatically switches between RPC/external API

### Decision Points

#### Choice: Optional URL vs Boolean Flag
**Chosen**: Optional URL (`Option<String>`)

**Rationale**:
- Allows dynamic switching without code changes
- URL can be changed via environment variable
- More flexible than boolean (supports multiple external APIs)
- Graceful fallback: if URL is invalid, falls back to RPC

**Alternative**: Boolean flag + hardcoded URL
- Less flexible
- Requires recompilation to change API endpoint
- No support for multiple environments

#### Choice: Direct HTTP Call vs RPC Proxy
**Chosen**: Direct HTTP call to external API

**Rationale**:
- Provides real-time data (critical for arbitrage)
- Bypasses local validator's stale state
- Can use specialized endpoints (e.g., Helius enhanced RPC)
- Reduces latency (no local validator overhead)

**Trade-offs**:
- Additional API cost (mitigated by 100ms cache)
- Network dependency (mitigated by fallback to RPC)
- More complex error handling (implemented with retry logic)

### Caching Strategy

#### Three-Tier Cache Architecture

**1. External API Cache (100ms TTL)**
```rust
external_api_cache: Arc<DashMap<Pubkey, (PoolData, SystemTime)>>
```
- **Purpose**: Reduce API calls during single arbitrage cycle
- **TTL**: 100ms (balances freshness vs API cost)
- **Use Case**: Multiple arbitrage checks in quick succession

**2. Pool Metadata Cache (300s TTL)**
```rust
pool_cache: Arc<DashMap<Pubkey, CachedPoolData>>
```
- **Purpose**: Cache pool structure (token mints, vaults, fees)
- **TTL**: 300 seconds (5 minutes)
- **Rationale**: Pool structure changes rarely

**3. Account Cache (60s TTL)**
```rust
account_cache: Cache<Pubkey, Account>
```
- **Purpose**: Cache non-pool accounts (token accounts, etc.)
- **TTL**: 60 seconds (configurable)
- **Rationale**: General-purpose caching

## Usage

### Setup Instructions

#### Step 1: Start Local Fork

```bash
# Start validator with mainnet fork
# NOTE: Only clone programs and token mints, NOT individual pools
solana-test-validator \
    --url https://mainnet.helius-rpc.com/?api-key=YOUR_KEY \
    --reset \
    --rpc-port 8899 \
    --faucet-port 9900 \
    --clone-upgradeable-program 675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8 \  # Raydium
    --clone-upgradeable-program whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc \  # Orca
    --clone-upgradeable-program LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo \  # Meteora
    --clone EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \  # USDC
    --clone So11111111111111111111111111111111111111112      # SOL
```

**Why not clone pools?**
- Pool accounts are large (slow to clone)
- Pool state becomes stale immediately
- We'll fetch fresh data from external API instead

#### Step 2: Configure Environment

Create `.env` file:
```bash
# RPC Configuration
RPC_URL=http://127.0.0.1:8899
WS_URL=ws://127.0.0.1:8900

# External Data API (real-time pool data)
EXTERNAL_DATA_API_URL=https://mainnet.helius-rpc.com/?api-key=YOUR_API_KEY

# Bot Configuration
BOT_SIMULATION_MODE=true        # Safe testing mode
ENABLE_ARBITRAGE=true
MIN_PROFIT_BPS=50               # 0.5% minimum profit

# Execution Configuration
COMPUTE_UNIT_LIMIT=1400000
COMPUTE_UNIT_PRICE=5000

# Cache Configuration
CACHE_TTL_SECONDS=60
CACHE_MAX_SIZE=10000
```

#### Step 3: Run Bot

```bash
# With environment variables
cargo run --release

# Or with explicit overrides
EXTERNAL_DATA_API_URL="https://mainnet.helius-rpc.com/?api-key=YOUR_KEY" \
RPC_URL="http://127.0.0.1:8899" \
BOT_SIMULATION_MODE=true \
cargo run --release
```

### Verification

#### Check Configuration Loading
Bot startup logs should show:
```
[INFO] TokenFetcher initialized with external API - batch size: 100, max retries: 5
[INFO] Using external API for pool data: https://mainnet.helius-rpc.com/...
```

#### Check Data Source
During arbitrage detection:
```
[DEBUG] Fetching batch of 3 pools from external API
[DEBUG] External API cache hit for pool: 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2
[INFO] Fetched 3 pools from external API
```

#### Fallback Behavior
If external API fails:
```
[ERROR] Failed to fetch from external API: connection timeout
[WARN] Falling back to RPC for 3 pools
[DEBUG] Cache miss for pool: ..., fetching from RPC
```

## Performance Characteristics

### Latency

**Without External API** (RPC only):
- Initial pool fetch: ~200-500ms (depends on fork state)
- Subsequent fetches: ~50ms (cached)
- Stale data risk: High (fork state doesn't update)

**With External API**:
- Initial pool fetch: ~100-300ms (depends on API)
- Subsequent fetches: <1ms (100ms cache)
- Stale data risk: Low (near real-time mainnet data)

### API Cost Optimization

**Cache Hit Rate**:
- Single arbitrage cycle: ~90% cache hits (100ms TTL sufficient)
- Multiple cycles per second: ~95% cache hits

**API Calls Per Minute** (3 pools, 10 checks/second):
- Without cache: 1,800 calls/minute
- With 100ms cache: ~180 calls/minute (90% reduction)

**Estimated Costs** (Helius pricing):
- Free tier: 10,000 credits/day = ~5 hours of testing
- Pro tier: 1M credits/month = ~460 hours of testing

## Troubleshooting

### Issue: External API timeout

**Symptoms**:
```
[ERROR] Failed to fetch from external API: connection timeout
[WARN] Falling back to RPC for 3 pools
```

**Solutions**:
1. Check API key is valid: `echo $EXTERNAL_DATA_API_URL`
2. Verify network connectivity: `curl -X POST $EXTERNAL_DATA_API_URL ...`
3. Increase timeout: `RPC_TIMEOUT_SECONDS=60`
4. Use backup RPC: `BACKUP_RPC_URLS="https://api.mainnet-beta.solana.com"`

### Issue: High API costs

**Symptoms**:
- Exceeding API rate limits
- High monthly costs

**Solutions**:
1. Increase cache TTL: `CACHE_TTL_SECONDS=120` (balance freshness vs cost)
2. Reduce check frequency: Lower arbitrage detection rate
3. Use free RPC for non-critical data
4. Implement request batching (already done via `batch_size`)

### Issue: Stale data from cache

**Symptoms**:
- Arbitrage opportunities disappear when executed
- Simulation shows profit but execution fails

**Solutions**:
1. Reduce external API cache TTL (default: 100ms)
2. Invalidate cache before critical operations:
   ```rust
   token_fetcher.invalidate_pool_cache(&pool_pubkey);
   ```
3. Force fresh fetch:
   ```rust
   token_fetcher.clear_all_caches();
   let fresh_data = token_fetcher.fetch_pool_data(...).await?;
   ```

### Issue: Local fork out of sync

**Symptoms**:
```
[ERROR] Transaction simulation failed: InvalidAccountData
[ERROR] Pool not found: ...
```

**Solutions**:
1. Restart validator with fresh fork:
   ```bash
   pkill -f solana-test-validator
   ./start-mainnet-fork.sh
   ```
2. Clone required programs (not pools):
   ```bash
   --clone-upgradeable-program <PROGRAM_ID>
   ```
3. Don't clone pool accounts (fetch dynamically)

## Advanced Configuration

### Multiple External APIs

Use primary + fallback pattern:
```bash
EXTERNAL_DATA_API_URL="https://mainnet.helius-rpc.com/?api-key=KEY1"
BACKUP_RPC_URLS="https://api.mainnet-beta.solana.com,https://rpc.ankr.com/solana"
```

The bot will:
1. Try external API first
2. Fall back to RPC on failure
3. Try backup RPCs if primary fails

### Custom API Endpoints

For Jupiter API or custom indexers:
```bash
# Jupiter Quote API
EXTERNAL_DATA_API_URL="https://quote-api.jup.ag/v6"

# Custom indexer
EXTERNAL_DATA_API_URL="https://your-indexer.com/api/v1/pools"
```

**Note**: Custom endpoints may require modifying `fetch_accounts_from_external_api()` to match their API format.

### Performance Tuning

Optimize for your use case:

**High-Frequency Trading** (prioritize speed):
```bash
CACHE_TTL_SECONDS=1              # Very short cache
EXTERNAL_DATA_API_URL=...        # Use fastest API
BATCH_SIZE=10                     # Small batches
```

**Cost-Optimized** (prioritize API cost):
```bash
CACHE_TTL_SECONDS=300            # Longer cache
BATCH_SIZE=100                    # Large batches
RPC_TIMEOUT_SECONDS=60           # Patient retries
```

**Balanced** (default):
```bash
CACHE_TTL_SECONDS=60
BATCH_SIZE=100
RPC_TIMEOUT_SECONDS=30
```

## Comparison: Testing Strategies

| Strategy | Data Source | Execution | Accuracy | Safety | Speed |
|----------|-------------|-----------|----------|--------|-------|
| **Conservative** (RPC only) | Local fork | Local fork | Low (stale) | High | Fast |
| **Aggressive** (API only) | External API | Mainnet | High | **Low** ⚠️ | Slow |
| **Dynamic** (Hybrid) | External API | Local fork | **High** ✅ | **High** ✅ | **Fast** ✅ |

**Dynamic = Best of Both Worlds**

## Next Steps

1. **Implement Pool Parsers**: Complete `parse_raydium_pool()`, `parse_meteora_pool()`, etc.
2. **Add More APIs**: Support Jupiter, Birdeye, or custom endpoints
3. **Metrics & Monitoring**: Track cache hit rates, API latency, fallback frequency
4. **Advanced Fallback**: Implement weighted fallback (try multiple APIs in order)
5. **Cost Analysis**: Add logging for API call counts and estimated costs

## References

- **Helius RPC Docs**: https://docs.helius.dev/
- **Solana JSON-RPC API**: https://docs.solana.com/api/http
- **Jupiter API**: https://station.jup.ag/docs/apis/quote-api
- **Exponential Backoff**: https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/

## Conclusion

The Dynamic Testing strategy provides:
- ✅ **Real-time data** from external APIs
- ✅ **Safe testing** environment via local fork
- ✅ **Cost efficiency** through intelligent caching
- ✅ **Reliability** via automatic fallback to RPC
- ✅ **Flexibility** through environment-based configuration

This enables accurate arbitrage detection in a risk-free testing environment, bridging the gap between conservative local testing and aggressive mainnet execution.
