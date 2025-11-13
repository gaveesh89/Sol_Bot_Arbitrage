# Meteora CPI Crates

This directory contains Rust crates for making Cross-Program Invocations (CPI) to Meteora Protocol programs on Solana.

## Overview

The Meteora CPI crates provide type-safe, high-level interfaces for interacting with Meteora's decentralized exchange infrastructure:

- **meteora-damm-cpi**: Dynamic Automated Market Maker (DAMM) pools
- **meteora-vault-cpi**: Yield-bearing vaults with locked profit tracking

## Crates

### meteora-damm-cpi

Provides CPI interfaces for Meteora DAMM pools, supporting multiple AMM curve types:
- Constant Product (Uniswap V2 style)
- Stable Swap (Curve style for stablecoins)
- Weighted pools

**Key Features:**
- Swap operations with slippage protection
- Liquidity provision (add/remove)
- Price calculations for different curve types
- Fee calculation helpers
- Pool state queries

**Helper Methods:**
```rust
impl Pool {
    fn calculate_swap_output(amount_in, source_is_token_a) -> u64
    fn get_price_a_to_b() -> f64
    fn calculate_lp_tokens_for_deposit(token_a, token_b) -> u64
    fn get_fee_bps() -> u64
    fn has_sufficient_liquidity(amount) -> bool
}
```

### meteora-vault-cpi

Provides CPI interfaces for Meteora yield vaults with sophisticated profit locking mechanisms.

**Key Features:**
- Deposit/withdraw with share-based accounting
- Locked profit degradation over time
- Harvest and compound operations
- APY estimation
- Time-weighted share price calculation

**Helper Methods:**
```rust
impl Vault {
    fn get_unlocked_amount(timestamp) -> u64
    fn convert_to_shares(assets, timestamp) -> u64
    fn convert_to_assets(shares, timestamp) -> u64
    fn get_share_price(timestamp) -> f64
    fn estimate_apy(timestamp) -> f64
}

impl LockedProfitTracker {
    fn calculate_locked_profit(timestamp, degradation) -> u64
    fn time_until_fully_unlocked(timestamp) -> i64
}
```

## Architecture

### Design Decisions

**✅ Chosen: Manual Struct Definitions**
- **Pros**: Full control over types and helpers, no build-time dependencies
- **Cons**: Must be updated if IDL changes

**Alternative: anchor-gen**
- **Pros**: Automatic code generation from IDL
- **Cons**: Requires IDL file, build-time complexity

### Structure

```
crates/
├── meteora-damm-cpi/
│   ├── Cargo.toml
│   ├── idl.json                # IDL specification
│   └── src/
│       └── lib.rs              # Pool structs, instructions, helpers
└── meteora-vault-cpi/
    ├── Cargo.toml
    ├── idl.json                # IDL specification
    └── src/
        └── lib.rs              # Vault structs, instructions, helpers
```

## Usage

### Adding to Your Project

Add to `Cargo.toml`:
```toml
[dependencies]
meteora-damm-cpi = { path = "crates/meteora-damm-cpi" }
meteora-vault-cpi = { path = "crates/meteora-vault-cpi" }
```

### Example: Swap on DAMM Pool

```rust
use meteora_damm_cpi::{Pool, build_swap_instruction_data};
use anchor_lang::prelude::*;

// Load pool account
let pool_account = /* fetch from chain */;
let pool = Pool::try_deserialize(&mut pool_account.data.as_ref())?;

// Calculate expected output
let amount_in = 1_000_000; // 1 token
let expected_out = pool.calculate_swap_output(amount_in, true)?;

// Apply slippage tolerance (1%)
let min_amount_out = (expected_out * 99) / 100;

// Build instruction
let ix_data = build_swap_instruction_data(amount_in, min_amount_out);

// Create CPI context and invoke...
```

### Example: Deposit to Vault

```rust
use meteora_vault_cpi::{Vault, build_deposit_instruction_data};
use anchor_lang::prelude::*;

// Load vault
let vault_account = /* fetch from chain */;
let vault = Vault::try_deserialize(&mut vault_account.data.as_ref())?;

// Calculate shares to receive
let deposit_amount = 10_000_000;
let current_time = Clock::get()?.unix_timestamp;
let shares = vault.convert_to_shares(deposit_amount, current_time)?;

// Check share price
let share_price = vault.get_share_price(current_time)?;
msg!("Depositing {} tokens for {} shares at price {}", 
     deposit_amount, shares, share_price);

// Build instruction
let ix_data = build_deposit_instruction_data(deposit_amount);

// Create CPI context and invoke...
```

## IDL Files

Each crate includes an `idl.json` file that defines:
- Program instructions and their parameters
- Account structures and fields
- Custom types and enums
- Error codes

### Updating from IDL

If the Meteora programs are updated:

1. Obtain the latest IDL from Meteora
2. Update `idl.json`
3. Regenerate or update Rust structs in `lib.rs`
4. Update helper methods if needed
5. Run tests: `cargo test --package <crate-name>`

## Testing

Both crates include comprehensive unit tests:

```bash
# Test DAMM CPI
cargo test --package meteora-damm-cpi

# Test Vault CPI
cargo test --package meteora-vault-cpi

# Test all CPI crates
cargo test --workspace --exclude solana-mev-bot
```

### Test Coverage

**meteora-damm-cpi:**
- ✅ Constant product swap calculations
- ✅ Price calculations
- ✅ Fee calculations
- ✅ LP token minting/burning

**meteora-vault-cpi:**
- ✅ Locked profit degradation
- ✅ Share <-> Asset conversions
- ✅ Share price calculations
- ✅ Time-based unlocking
- ✅ APY estimation

## Helper Methods

### Why Custom Helpers?

The generated CPI code from Anchor only provides instruction builders. We add custom helper methods to:

1. **Simplify bot logic** - Calculate expected outputs before execution
2. **Reduce RPC calls** - Compute values locally when possible
3. **Type safety** - Strong typing for financial calculations
4. **Testing** - Unit testable without blockchain interaction

### Optimization

**Minimal Helpers**: Only implement helpers that are:
- Frequently used in bot logic
- Computationally expensive to fetch from chain
- Critical for decision-making (price, slippage, fees)

**Not Implemented**: Complex helpers that would bloat the code without clear benefit.

## Program IDs

Current placeholder IDs in `declare_id!`:
- **meteora-damm**: `LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo`
- **meteora-vault**: `24Uqj9JCLxUeoC3hGfh5W3s9FM9uCHDS2SG3LYwBpyTi`

**⚠️ Replace with actual Meteora program IDs before deployment!**

## Error Handling

Both crates define custom error codes aligned with Meteora's on-chain errors:

```rust
#[error_code]
pub enum ErrorCode {
    InvalidCurveType,
    InsufficientLiquidity,
    SlippageExceeded,
    InvalidFeeParameters,
    MathOverflow,
    // ...
}
```

## Performance

- **Zero-copy deserialization**: Uses Anchor's zero-copy traits where possible
- **Stack-allocated**: Structs are sized for stack allocation
- **No heap allocations**: Helper methods avoid unnecessary allocations
- **Overflow checks**: All math operations check for overflow

## Security Considerations

1. **Slippage Protection**: Always calculate and enforce minimum output amounts
2. **Overflow Protection**: All arithmetic uses checked operations
3. **Account Validation**: Verify account ownership and program IDs
4. **Reentrancy**: Follow Anchor's CPI best practices

## Contributing

When adding new helpers:

1. Add the method to the appropriate `impl` block
2. Document with rustdoc comments
3. Add unit tests
4. Update this README with examples

## License

Same as parent project (see root LICENSE file)

## Resources

- [Meteora Documentation](https://docs.meteora.ag/)
- [Anchor CPI Guide](https://www.anchor-lang.com/docs/cross-program-invocations)
- [Solana Program Library](https://spl.solana.com/)

---

**Built for the Solana MEV Bot project**
