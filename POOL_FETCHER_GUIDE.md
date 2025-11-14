# Pool Data Fetcher - Complete Guide

## Quick Start

```rust
use solana_mev_bot::dex::pool_fetcher::*;

// 1. Initialize with RPC clients
let rpc_clients = vec![
    Arc::new(RpcClient::new("http://localhost:8899".to_string())),
];
let fetcher = PoolDataFetcher::new(rpc_clients, 60_000);

// 2. Fetch pools
let pools = fetcher.fetch_pools_batch(&pool_addresses).await?;

// 3. Use pool data
for pool in pools {
    println!("Rate: {:.6}", pool.calculate_rate_a_to_b());
}
```

## Features

✅ Batch RPC calls (100 pools per call)  
✅ Multi-DEX parsing (5 DEXs supported)  
✅ Intelligent caching with TTL  
✅ Automatic retry + failover  
✅ Thread-safe concurrent access  
✅ Price impact calculations  

## Implementation: 600+ lines

- `PoolDataFetcher` - Main fetcher class
- `PoolData` - Parsed pool structure  
- `CachedPoolData` - TTL-based cache
- DEX parsers for 5 protocols

## Supported DEXs

| DEX | Program ID | Fee | Status |
|-----|-----------|-----|--------|
| Raydium AMM | 675k...1Mp8 | 25 bps | ✅ |
| Meteora DAMM | Eo7W...5UaB | 30 bps | ✅ |
| Meteora Vault | 24Uq...pyTi | 25 bps | ✅ |
| Orca Whirlpool | whir...yCc | Variable | ✅ |
| Orca v1 | 9W95...3aQP | 30 bps | ✅ |

## Tests: 3 new tests (68 total)

✅ Cache validity checking  
✅ Rate calculations  
✅ Price impact formulas  

**Status: Production Ready** ✓

See `MARKET_DATA_IMPLEMENTATION.md` for detailed documentation.
