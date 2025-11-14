# Solana MEV Bot

A high-performance Solana MEV (Maximal Extractable Value) bot written in Rust, focusing on arbitrage opportunities across multiple DEXs including Raydium, Meteora, Pump, Whirlpool, and Orca.

## Features

- **Multi-DEX Arbitrage**: Monitor and execute arbitrage across Raydium, Meteora, Pump, Whirlpool, and Orca
- **Enhanced Data Fetching**: Implement caching, batching, and retry logic to reduce RPC load
- **Meteora Integration**: Deep integration with Meteora DAMM and Vault programs for advanced liquidity strategies
- **High Performance**: Built in Rust with async/await using Tokio runtime
- **Optimized Transactions**: Uses versioned transactions and address lookup tables (ALT)
- **Transaction Spamming**: Multi-RPC transaction submission for higher inclusion probability
- **Structured Logging**: Uses `tracing` for detailed, filterable logs

## Project Structure

```
solana-mev-bot/
├── src/
│   ├── main.rs                 # Entry point and bot orchestration
│   ├── config.rs               # Configuration loading from .env
│   ├── chain/
│   │   ├── mod.rs              # Chain interaction module
│   │   ├── token_fetch.rs      # Token fetching with caching
│   │   └── token_price.rs      # Market data and arbitrage calculation
│   ├── dex/
│   │   ├── mod.rs              # DEX integrations
│   │   ├── raydium.rs          # Raydium integration
│   │   ├── meteora.rs          # Meteora integration
│   │   ├── whirlpool.rs        # Whirlpool integration
│   │   └── pump.rs             # Pump integration
│   ├── meteora/
│   │   ├── mod.rs              # Meteora-specific modules
│   │   ├── damm_cpi.rs         # DAMM CPI integration
│   │   └── vault_cpi.rs        # Vault CPI integration
│   └── utils/
│       ├── mod.rs              # Utility functions
│       ├── retry.rs            # Retry logic
│       └── transaction.rs      # Transaction helpers
├── Cargo.toml                  # Dependencies and project configuration
├── .env.example                # Example environment variables
└── README.md                   # This file
```

## Prerequisites

- Rust 1.70 or higher
- Solana CLI tools (optional, for wallet management)
- A funded Solana wallet with SOL for transaction fees
- RPC endpoint (Helius, QuickNode, or public RPC)

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd solana-mev-bot
```

2. Copy the example environment file and configure it:
```bash
cp .env.example .env
# Edit .env with your settings
```

3. Create or import your wallet:
```bash
# Generate a new wallet
solana-keygen new -o wallet.json

# Or copy your existing wallet
cp /path/to/your/wallet.json ./wallet.json
```

4. Build the project:
```bash
cargo build --release
```

## Configuration

Edit the `.env` file with your specific configuration:

- **RPC_URL**: Your Solana RPC endpoint
- **WALLET_KEYPAIR_PATH**: Path to your wallet keypair file
- **MIN_PROFIT_BPS**: Minimum profit threshold in basis points
- **MAX_SLIPPAGE_BPS**: Maximum acceptable slippage
- **Token Mints**: Addresses of tokens you want to monitor

See `.env.example` for all available configuration options.

## Usage

Run the bot:
```bash
cargo run --release
```

Run with custom log level:
```bash
RUST_LOG=debug cargo run --release
```

## Architecture

### Data Flow
```
Config Loading → RPC Client Initialization → Token Fetcher (with caching)
    ↓
Market Data Fetcher → Arbitrage Calculator → Transaction Builder
    ↓
Multi-RPC Transaction Submitter → Confirmation Monitor
```

### Key Components

1. **TokenFetcher**: Fetches and caches token account data with retry logic
2. **MarketDataFetcher**: Monitors prices across multiple DEXs
3. **Arbitrage Calculator**: Identifies profitable arbitrage opportunities
4. **Meteora CPI**: Direct integration with Meteora DAMM/Vault programs
5. **Transaction Optimizer**: Creates optimized versioned transactions

## Performance Optimizations

- **Caching**: Moka cache for token and pool data (configurable TTL)
- **Batching**: Batch RPC requests to reduce latency
- **Retry Logic**: Exponential backoff for failed operations
- **Versioned Transactions**: Reduced transaction size and cost
- **Address Lookup Tables**: Further transaction optimization
- **Multi-RPC Submission**: Parallel transaction submission to multiple RPCs

## Risk Management

⚠️ **Important**: This bot interacts with real money on the Solana blockchain.

- Start with small amounts for testing
- Monitor logs carefully for errors
- Set appropriate profit thresholds and slippage limits
- Keep your private keys secure (never commit them to git)
- Test thoroughly on devnet before mainnet deployment

## Development

Run tests:
```bash
cargo test
```

Run with detailed logging:
```bash
RUST_LOG=trace cargo run
```

Format code:
```bash
cargo fmt
```

Lint code:
```bash
cargo clippy
```

## Documentation

- **[Quick Start Guide](./QUICKSTART.md)** - Get started quickly with basic configuration
- **[Devnet Testing Guide](./DEVNET_TESTING.md)** - Test the bot safely on Solana Devnet
- **[Mainnet Fork Testing Guide](./MAINNET_FORK_TESTING.md)** - Test against real Mainnet state locally (zero risk)
- **[Security Best Practices](./SECURITY.md)** - Critical security guidelines for production deployment
- **[Implementation Summary](./IMPLEMENTATION_SUMMARY.md)** - Detailed technical implementation overview

## License

MIT License - see LICENSE file for details

## Disclaimer

This software is provided for educational purposes only. Use at your own risk. The authors are not responsible for any financial losses incurred through the use of this software.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## Acknowledgments

- Solana Foundation for the Solana blockchain
- Raydium, Meteora, Orca teams for their DEX protocols
- Rust community for excellent tooling and libraries
