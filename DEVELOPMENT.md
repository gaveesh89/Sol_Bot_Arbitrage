# Development Guide

## Project Structure

```
src/
├── main.rs              # Entry point, bot orchestration
├── config.rs            # Configuration management
├── chain/               # Blockchain interaction
│   ├── token_fetch.rs   # Token data fetching with caching
│   └── token_price.rs   # Price monitoring and arbitrage detection
├── dex/                 # DEX-specific integrations
│   ├── raydium.rs       # Raydium integration
│   ├── meteora.rs       # Meteora DLMM integration
│   ├── whirlpool.rs     # Whirlpool integration
│   └── pump.rs          # Pump integration
├── meteora/             # Meteora advanced features
│   ├── damm_cpi.rs      # Dynamic AMM CPI
│   └── vault_cpi.rs     # Vault CPI
└── utils/               # Utility functions
    ├── retry.rs         # Retry logic with backoff
    └── transaction.rs   # Transaction building and sending
```

## Adding a New DEX

To add support for a new DEX:

1. Create a new file in `src/dex/` (e.g., `orca.rs`)
2. Implement the client with swap and liquidity functions
3. Add pool parsing logic in `src/chain/token_fetch.rs`
4. Update `DexType` enum in `token_fetch.rs`
5. Add the DEX to `src/dex/mod.rs`

Example structure:

```rust
pub struct OrcaClient {
    rpc_client: Arc<RpcClient>,
    program_id: Pubkey,
}

impl OrcaClient {
    pub fn new(rpc_client: Arc<RpcClient>, program_id: Pubkey) -> Self {
        Self { rpc_client, program_id }
    }

    pub async fn swap(&self, pool: &Pubkey, amount: u64) -> Result<String> {
        // Implementation
    }
}
```

## Implementing Pool Parsers

Each DEX has its own pool account structure. To parse pools:

1. Study the DEX's on-chain program structure (use Anchor IDL if available)
2. Define the account structure using `borsh` or `bincode`
3. Implement parsing in the appropriate `parse_*_pool` function

Example:

```rust
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
struct RaydiumPoolState {
    pub status: u64,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_vault: Pubkey,
    pub token_b_vault: Pubkey,
    // ... other fields
}

fn parse_raydium_pool(&self, account: &Account) -> Result<PoolData> {
    let pool_state = RaydiumPoolState::try_from_slice(&account.data)?;
    // Convert to PoolData
}
```

## Testing

### Unit Tests

Run unit tests:
```bash
cargo test
```

Run specific test:
```bash
cargo test test_name
```

### Integration Tests

Create integration tests in `tests/` directory:

```rust
#[tokio::test]
async fn test_price_fetching() {
    // Test implementation
}
```

### Devnet Testing

1. Set RPC_URL to devnet:
   ```
   RPC_URL=https://api.devnet.solana.com
   ```

2. Get devnet SOL:
   ```bash
   solana airdrop 2 --url devnet
   ```

3. Run the bot on devnet first!

## Performance Optimization

### Caching

The bot uses Moka cache for:
- Token account data
- Pool data
- Price information

Configure cache TTL and size in `.env`:
```
CACHE_TTL_SECONDS=60
CACHE_MAX_SIZE=10000
```

### Transaction Optimization

1. **Compute Budget**: Set appropriate compute units
   ```rust
   tx_builder.set_compute_unit_limit(200000);
   tx_builder.set_compute_unit_price(1000);
   ```

2. **Versioned Transactions**: Use for complex transactions
   ```rust
   let versioned_tx = tx_builder.build_versioned(
       &rpc_client,
       &signer,
       lookup_tables
   ).await?;
   ```

3. **Multi-RPC Submission**: Send to multiple RPCs
   ```rust
   let multi_rpc = MultiRpcSender::new(rpc_urls);
   multi_rpc.send_transaction_multiple(&tx).await?;
   ```

### Batching

Fetch multiple accounts in one call:
```rust
let accounts = token_fetcher
    .fetch_accounts_batch(&pubkeys)
    .await?;
```

## Logging

The bot uses `tracing` for structured logging:

```rust
use tracing::{info, warn, error, debug};

info!("Transaction sent: {}", signature);
warn!("Low balance detected");
error!("Failed to fetch price: {}", error);
debug!("Cache hit for account: {}", pubkey);
```

Set log level in `.env`:
```
RUST_LOG=info,mev_bot=debug
```

## Security Best Practices

1. **Never commit private keys**
   - Keep wallet.json in .gitignore
   - Use environment variables for sensitive data

2. **Validate all inputs**
   - Check pool addresses before swapping
   - Verify token mints
   - Validate amounts

3. **Set safety limits**
   - Minimum profit threshold
   - Maximum slippage
   - Trade size limits

4. **Monitor transactions**
   - Log all transactions
   - Monitor for failed transactions
   - Set up alerts for anomalies

## Troubleshooting

### Build Errors

If you encounter build errors:

```bash
# Update Rust
rustup update

# Clean and rebuild
cargo clean
cargo build --release
```

### RPC Errors

If RPC calls fail:
- Check your RPC endpoint is working
- Verify rate limits aren't exceeded
- Try backup RPC endpoints
- Consider using a premium RPC service (Helius, QuickNode)

### Transaction Failures

Common causes:
- Insufficient balance for fees
- Slippage exceeded
- Pool liquidity too low
- Compute budget exceeded
- Stale blockhash

## Monitoring and Metrics

To enable Prometheus metrics:

1. Enable in `.env`:
   ```
   ENABLE_METRICS=true
   ```

2. Build with metrics feature:
   ```bash
   cargo build --release --features metrics
   ```

3. Access metrics at `http://localhost:9090/metrics`

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## Resources

- [Solana Documentation](https://docs.solana.com/)
- [Anchor Framework](https://www.anchor-lang.com/)
- [Raydium Protocol](https://raydium.io/)
- [Meteora Documentation](https://docs.meteora.ag/)
- [Orca Whirlpools](https://www.orca.so/)
