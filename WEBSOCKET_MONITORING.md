# WebSocket-Based Real-Time Pool Monitoring

## Overview

The WebSocket pool monitoring system provides real-time updates for Solana liquidity pool accounts using `accountSubscribe` subscriptions. This enables millisecond-level arbitrage detection by immediately responding to pool state changes rather than polling.

## Architecture

### Components

1. **PoolMonitor**: Core monitoring struct that manages WebSocket subscriptions for a set of pool addresses
2. **MonitorConfig**: Configuration for rate limiting and reconnection behavior
3. **PoolUpdate**: Event structure containing pool state changes
4. **BatchPoolMonitor**: Shards subscriptions across multiple WebSocket URLs for scalability

### Event Flow

```
Pool Account Changes (on-chain)
    ↓
WebSocket accountSubscribe
    ↓
PoolUpdate Event
    ↓
ArbitrageGraph Update (bidirectional edges)
    ↓
Rate-Limited Detection (throttled to prevent CPU overload)
    ↓
Arbitrage Opportunities
```

## Usage

### Basic Usage

```rust
use crate::chain::pool_monitor::{PoolMonitor, MonitorConfig};
use crate::dex::triangular_arb::{create_shared_graph, BellmanFordDetector};
use crate::dex::pool_fetcher::PoolDataFetcher;
use solana_sdk::pubkey::Pubkey;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize components
    let graph = create_shared_graph();
    let detector = BellmanFordDetector::new(graph.clone());
    let pool_fetcher = PoolDataFetcher::new(vec![], 10_000);
    
    // Configure monitoring
    let config = MonitorConfig {
        ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
        rate_limit_ms: 1000,  // Trigger detection at most once per second
        reconnect_delay_ms: 2000,
        max_reconnect_attempts: 10,
        subscription_batch_size: 50,
    };
    
    // Set up pool addresses to monitor
    let pool_addresses = vec![
        Pubkey::from_str("POOL_ADDRESS_1")?,
        Pubkey::from_str("POOL_ADDRESS_2")?,
        // ... more pools
    ];
    
    // Create and start monitor
    let monitor = PoolMonitor::new(
        config,
        pool_addresses,
        graph,
        detector,
        pool_fetcher,
    );
    
    // Start monitoring (blocks until error or shutdown)
    monitor.start_monitoring().await?;
    
    Ok(())
}
```

### Advanced: Multi-WebSocket Sharding

For large numbers of pools (>1000), use `BatchPoolMonitor` to distribute subscriptions across multiple WebSocket connections:

```rust
use crate::chain::pool_monitor::{BatchPoolMonitor, MonitorConfig};

let ws_urls = vec![
    "wss://api.mainnet-beta.solana.com",
    "wss://solana-api.projectserum.com",
    "wss://rpc.ankr.com/solana",
];

let batch_monitor = BatchPoolMonitor::new(
    ws_urls.into_iter().map(|s| s.to_string()).collect(),
    pool_addresses,
    graph,
    detector,
    pool_fetcher,
    config,
);

batch_monitor.start_all().await?;
```

## Configuration

### MonitorConfig Options

```rust
pub struct MonitorConfig {
    /// WebSocket URL for Solana RPC
    pub ws_url: String,
    
    /// Minimum time between arbitrage detection runs (milliseconds)
    /// Recommended: 1000ms to prevent CPU overload
    pub rate_limit_ms: u64,
    
    /// Delay between reconnection attempts (milliseconds)
    pub reconnect_delay_ms: u64,
    
    /// Maximum number of reconnection attempts before giving up
    pub max_reconnect_attempts: usize,
    
    /// Number of pools to subscribe to per batch
    /// Recommended: 50-100 to avoid overwhelming single WebSocket
    pub subscription_batch_size: usize,
}
```

### Recommended Settings

**Development/Testing:**
```rust
MonitorConfig {
    ws_url: "wss://api.devnet.solana.com".to_string(),
    rate_limit_ms: 2000,  // Slower for easier debugging
    reconnect_delay_ms: 5000,
    max_reconnect_attempts: 3,
    subscription_batch_size: 10,
}
```

**Production (Low-Medium Volume):**
```rust
MonitorConfig {
    ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
    rate_limit_ms: 1000,  // 1 detection per second
    reconnect_delay_ms: 2000,
    max_reconnect_attempts: 10,
    subscription_batch_size: 50,
}
```

**Production (High Volume):**
```rust
MonitorConfig {
    ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
    rate_limit_ms: 500,  // 2 detections per second
    reconnect_delay_ms: 1000,
    max_reconnect_attempts: 15,
    subscription_batch_size: 100,
}
```

## How It Works

### 1. Subscription Management

The monitor creates WebSocket subscriptions using Solana's `accountSubscribe` RPC method:

```rust
pubsub_client.account_subscribe(
    pool_address,
    Some(RpcAccountInfoConfig {
        encoding: Some(UiAccountEncoding::Base64),
        commitment: Some(CommitmentConfig::confirmed()),
        data_slice: None,
        min_context_slot: None,
    })
)
```

### 2. Update Processing

When a pool account changes:

1. **Receive**: WebSocket stream emits `Response<UiAccount>`
2. **Decode**: Convert `UiAccount` to `Account`
3. **Parse**: Extract pool-specific data (reserves, fees, etc.)
4. **Update Graph**: Modify both A→B and B→A edges with new rates
5. **Trigger Detection**: If rate limit allows, run Bellman-Ford

### 3. Rate Limiting

Updates are accumulated and detection is throttled:

```rust
let mut last_detection = Instant::now();
let rate_limit = Duration::from_millis(config.rate_limit_ms);

loop {
    // Accumulate updates...
    
    if last_detection.elapsed() >= rate_limit {
        // Run detection
        detector.detect_arbitrage(base_currency, max_path_length).await;
        last_detection = Instant::now();
    }
}
```

This prevents CPU overload from rapid updates while ensuring timely opportunity detection.

### 4. Reconnection Logic

Automatic reconnection with exponential backoff:

```rust
let mut attempt = 0;
let mut delay = Duration::from_millis(config.reconnect_delay_ms);

while attempt < config.max_reconnect_attempts {
    match subscribe_all_pools().await {
        Ok(_) => break,
        Err(e) => {
            warn!("Reconnect attempt {} failed: {}", attempt + 1, e);
            tokio::time::sleep(delay).await;
            delay *= 2;  // Exponential backoff
            attempt += 1;
        }
    }
}
```

## Performance Characteristics

### Latency

- **WebSocket Update**: 10-50ms (network + Solana processing)
- **Account Decode**: <1ms
- **Graph Update**: <5ms (concurrent write lock)
- **Detection**: 50-500ms (depends on graph size)
- **Total**: ~100-600ms from on-chain change to detected opportunity

### Resource Usage

**Memory:**
- ~50KB per pool subscription
- 1000 pools ≈ 50MB overhead

**CPU:**
- Update processing: <1% per pool change
- Detection: 5-50% per run (varies with graph size)
- Rate limiting is critical to prevent 100% CPU usage

**Network:**
- ~100 bytes per update
- 10 updates/sec/pool ≈ 1KB/sec/pool
- 1000 pools ≈ 1MB/sec

### Scalability

| Pools | WebSocket Connections | Detection Rate | CPU Usage |
|-------|---------------------|----------------|-----------|
| 100   | 1                   | 1000ms         | 10-20%    |
| 500   | 1                   | 1000ms         | 20-40%    |
| 1000  | 2-3                 | 1000ms         | 40-60%    |
| 5000  | 10-15               | 1000ms         | 70-90%    |

**Recommendations:**
- Use `BatchPoolMonitor` for >500 pools
- Consider multiple bot instances for >5000 pools
- Monitor CPU usage and adjust `rate_limit_ms` accordingly

## Monitoring and Debugging

### Statistics

Access monitoring statistics:

```rust
let stats = monitor.get_stats();
println!("Updates received: {}", stats.total_updates);
println!("Detection runs: {}", stats.detection_runs);
println!("Active subscriptions: {}", stats.active_subscriptions);
```

### Logging

Set log levels for visibility:

```rust
env_logger::Builder::from_default_env()
    .filter_module("mev_bot::chain::pool_monitor", log::LevelFilter::Debug)
    .init();
```

Key log messages:
- `INFO`: Subscription lifecycle events
- `DEBUG`: Individual pool updates
- `WARN`: Reconnection attempts, decode failures
- `ERROR`: Critical failures requiring intervention

### Common Issues

**Issue:** High CPU usage

**Solution:** Increase `rate_limit_ms` to reduce detection frequency

---

**Issue:** Subscriptions disconnecting frequently

**Solution:** 
1. Check network stability
2. Increase `max_reconnect_attempts`
3. Use different `ws_url` (e.g., private RPC node)

---

**Issue:** Missing updates for some pools

**Solution:**
1. Verify pool addresses are correct
2. Check `subscription_batch_size` isn't too large
3. Monitor for WebSocket connection limits

---

**Issue:** Slow arbitrage detection

**Solution:**
1. Reduce `rate_limit_ms` (if CPU allows)
2. Optimize graph traversal (reduce nodes/edges)
3. Use faster hardware or optimize Bellman-Ford implementation

## Integration with Other Components

### Pool Data Fetcher

Initial pool state is loaded via `PoolDataFetcher`, then maintained via WebSocket:

```rust
// 1. Fetch initial state
let pools = pool_fetcher.fetch_all_pools().await?;

// 2. Build graph
for pool in pools {
    graph.add_edge(pool.token_a, pool.token_b, exchange_rate, ...);
}

// 3. Start monitoring for updates
monitor.start_monitoring().await?;
```

### Triangular Arbitrage

Graph updates automatically trigger detection:

```rust
// In process_updates():
graph.update_edge(&token_a, &token_b, new_rate, ...);
graph.update_edge(&token_b, &token_a, reverse_rate, ...);

if last_detection.elapsed() >= rate_limit {
    let opportunities = detector.detect_arbitrage(base_currency, 4).await?;
    // Execute profitable paths...
}
```

### Execution Pipeline

Complete flow from monitoring to execution:

```
WebSocket Update → Graph Update → Detection → Validation → Execution
     ↓                  ↓             ↓            ↓           ↓
  100-600ms          <5ms        50-500ms      10-50ms     500-2000ms
```

## Testing

### Unit Tests

Run pool monitor tests:

```bash
cargo test pool_monitor
```

### Integration Testing

Test with devnet:

```rust
// Use devnet WebSocket
let config = MonitorConfig {
    ws_url: "wss://api.devnet.solana.com".to_string(),
    // ... other config
};

// Monitor known test pools
let test_pools = vec![
    Pubkey::from_str("DEVNET_POOL_1")?,
    // ...
];

monitor.start_monitoring().await?;
```

### Mainnet Fork Testing

Test with mainnet-fork:

```bash
# Start mainnet fork
./start-mainnet-fork.sh

# Run bot with fork URL
SOLANA_RPC_URL=http://localhost:8899 cargo run
```

## Future Enhancements

### Planned Features

1. **Filter-based Subscriptions**: Subscribe to program accounts with filters to automatically discover new pools
2. **Adaptive Rate Limiting**: Adjust detection frequency based on CPU usage
3. **Subscription Health Monitoring**: Automatic fallback to backup WebSocket URLs
4. **Update Compression**: Batch multiple updates before graph modification
5. **Priority Pools**: Different rate limits for high-volume vs low-volume pools

### Optimization Opportunities

1. **Parallel Detection**: Run detection on subset of graph affected by updates
2. **Incremental Updates**: Only re-run detection on paths involving updated pools
3. **WebSocket Pooling**: Reuse connections across multiple monitors
4. **Zero-Copy Parsing**: Parse pool data without allocation where possible

## Security Considerations

### WebSocket Security

- Use WSS (TLS) for all connections
- Validate certificate chains in production
- Consider IP whitelisting for private RPC nodes

### DoS Protection

- Rate limiting prevents CPU exhaustion
- Max reconnect attempts prevent infinite loops
- Subscription limits prevent memory exhaustion

### Data Validation

- Validate all decoded account data
- Sanity check pool reserves (no zero/negative values)
- Verify account owners match expected programs

## Conclusion

The WebSocket monitoring system is the critical real-time component of the MEV bot. Proper configuration of rate limiting and reconnection parameters ensures reliable operation while maximizing profitability by minimizing detection latency.

For questions or issues, refer to the logs, adjust configuration parameters, and consider the performance characteristics documented above.
