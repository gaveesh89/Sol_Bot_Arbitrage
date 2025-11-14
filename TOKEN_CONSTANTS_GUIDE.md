# Token Mint Constants - Usage Guide

## What Was Implemented

Created `src/chain/constants.rs` with three compile-time constants for well-known Solana tokens:

- **USDC_MINT**: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` (6 decimals)
- **USDT_MINT**: `Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB` (6 decimals)
- **WSOL_MINT**: `So11111111111111111111111111111111111111112` (9 decimals)

## Test Results

✅ **All 45 tests passing** (4 new constants tests + 41 existing)
✅ **Clean compilation**
✅ **Compile-time address validation** using `solana_program::pubkey!` macro

## Usage Examples

### 1. Import in Your Code

```rust
use crate::chain::{USDC_MINT, USDT_MINT, WSOL_MINT};
```

### 2. Check if Token is a Stablecoin

```rust
pub fn is_stablecoin(mint: &Pubkey) -> bool {
    mint == &USDC_MINT || mint == &USDT_MINT
}
```

### 3. Filter for Major Tokens

```rust
pub fn is_major_token(mint: &Pubkey) -> bool {
    matches!(mint, &USDC_MINT | &USDT_MINT | &WSOL_MINT)
}
```

### 4. Prioritize Trading Pairs

```rust
pub fn should_prioritize_pair(token_a: &Pubkey, token_b: &Pubkey) -> bool {
    // Prioritize pairs involving USDC or WSOL
    [token_a, token_b].iter().any(|&mint| {
        mint == &USDC_MINT || mint == &WSOL_MINT
    })
}
```

### 5. Token Display Names

```rust
pub fn get_token_symbol(mint: &Pubkey) -> &'static str {
    match mint {
        &USDC_MINT => "USDC",
        &USDT_MINT => "USDT",
        &WSOL_MINT => "WSOL",
        _ => "UNKNOWN",
    }
}
```

### 6. Risk Assessment by Token

```rust
pub fn get_token_risk_level(mint: &Pubkey) -> RiskLevel {
    match mint {
        &USDC_MINT | &USDT_MINT => RiskLevel::Low,  // Stablecoins = low risk
        &WSOL_MINT => RiskLevel::Medium,             // Native token = medium risk
        _ => RiskLevel::High,                        // Unknown tokens = high risk
    }
}
```

### 7. In Opportunity Detection

```rust
// In your arbitrage opportunity detection
pub fn detect_opportunities(&self) -> Vec<ArbitrageOpportunity> {
    let mut opportunities = Vec::new();
    
    // Prioritize USDC/WSOL pairs
    let priority_pairs = [
        (USDC_MINT, WSOL_MINT),
        (USDT_MINT, WSOL_MINT),
        (USDC_MINT, USDT_MINT),
    ];
    
    for (token_a, token_b) in priority_pairs {
        if let Some(opp) = self.check_pair(&token_a, &token_b) {
            opportunities.push(opp);
        }
    }
    
    opportunities
}
```

### 8. In Profit Calculation

```rust
pub fn calculate_profit_usd(&self, profit_amount: u64, profit_token: &Pubkey) -> f64 {
    let price_usd = match profit_token {
        &USDC_MINT | &USDT_MINT => 1.0,  // Stablecoins = $1
        &WSOL_MINT => self.get_sol_price(),
        _ => self.get_token_price(profit_token),
    };
    
    let decimals = match profit_token {
        &USDC_MINT | &USDT_MINT => 6,
        &WSOL_MINT => 9,
        _ => 9,  // Default
    };
    
    (profit_amount as f64 / 10_f64.powi(decimals)) * price_usd
}
```

### 9. In Token Fetcher

```rust
// In src/chain/token_fetch.rs
use crate::chain::{USDC_MINT, USDT_MINT, WSOL_MINT};

impl TokenFetcher {
    pub async fn fetch_major_tokens(&self) -> Result<Vec<TokenAccount>> {
        let mints = vec![USDC_MINT, USDT_MINT, WSOL_MINT];
        let mut accounts = Vec::new();
        
        for mint in mints {
            if let Ok(account) = self.fetch_token_account(&mint).await {
                accounts.push(account);
            }
        }
        
        Ok(accounts)
    }
}
```

### 10. In Reporting

```rust
// In src/reporting/mod.rs
use crate::chain::{USDC_MINT, WSOL_MINT};

pub fn format_profit_summary(record: &TradeRecord) -> String {
    let token_symbol = match &record.profit_token_mint.parse::<Pubkey>() {
        Ok(mint) if mint == &USDC_MINT => "USDC",
        Ok(mint) if mint == &WSOL_MINT => "SOL",
        _ => "tokens",
    };
    
    format!("Profit: {} {}", record.profit_amount, token_symbol)
}
```

## Integration Examples

### A. Enhanced Opportunity Filtering

```rust
// Add to your opportunity detection logic
pub struct OpportunityFilter {
    // ... existing fields
}

impl OpportunityFilter {
    pub fn filter_by_token_quality(&self, opps: Vec<ArbitrageOpportunity>) -> Vec<ArbitrageOpportunity> {
        opps.into_iter()
            .filter(|opp| {
                // Only trade pairs involving major tokens
                let token_a = opp.token_a_mint.parse::<Pubkey>().ok();
                let token_b = opp.token_b_mint.parse::<Pubkey>().ok();
                
                if let (Some(a), Some(b)) = (token_a, token_b) {
                    [a, b].iter().any(|mint| {
                        mint == &USDC_MINT || 
                        mint == &USDT_MINT || 
                        mint == &WSOL_MINT
                    })
                } else {
                    false
                }
            })
            .collect()
    }
}
```

### B. Configuration Validation

```rust
// Validate that config tokens are known
pub fn validate_config(config: &Config) -> Result<()> {
    let known_mints = [USDC_MINT, USDT_MINT, WSOL_MINT];
    
    if let Some(ref preferred_token) = config.preferred_profit_token {
        let mint = preferred_token.parse::<Pubkey>()?;
        if !known_mints.contains(&mint) {
            tracing::warn!("Configured token {} is not a known major token", preferred_token);
        }
    }
    
    Ok(())
}
```

### C. Dynamic Token Selection

```rust
// Select best token for profit based on liquidity
pub async fn select_best_profit_token(&self, amount: u64) -> Pubkey {
    let tokens = [
        (USDC_MINT, self.get_usdc_liquidity().await),
        (USDT_MINT, self.get_usdt_liquidity().await),
        (WSOL_MINT, self.get_wsol_liquidity().await),
    ];
    
    // Select token with highest liquidity
    tokens.into_iter()
        .max_by_key(|(_, liquidity)| *liquidity)
        .map(|(mint, _)| mint)
        .unwrap_or(USDC_MINT)  // Default to USDC
}
```

## Benefits

### 1. **Compile-Time Safety**
```rust
// This will fail at compile time if address is invalid:
pub const INVALID: Pubkey = solana_program::pubkey!("invalid_address");
// Error: invalid public key
```

### 2. **Performance**
- Zero runtime overhead
- No parsing or allocation
- Inlined at compile time

### 3. **Readability**
```rust
// Before:
if token == "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse().unwrap() {

// After:
if token == USDC_MINT {
```

### 4. **Type Safety**
```rust
// Constants are already Pubkey type, no parsing needed
fn process_usdc_payment(mint: Pubkey) -> Result<()> {
    assert_eq!(mint, USDC_MINT);
    // Process...
}
```

## Adding More Constants

To add more well-known tokens:

```rust
// In src/chain/constants.rs

/// Jupiter token mint
pub const JUP_MINT: Pubkey = solana_program::pubkey!("JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN");

/// Pyth token mint
pub const PYTH_MINT: Pubkey = solana_program::pubkey!("HZ1JovNiVvGrGNiiYvEozEVgZ58xaU3RKwX8eACQBCt3");

/// Raydium token mint
pub const RAY_MINT: Pubkey = solana_program::pubkey!("4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R");

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_all_governance_tokens() {
        let tokens = [JUP_MINT, PYTH_MINT, RAY_MINT];
        // Ensure all are unique
        for i in 0..tokens.len() {
            for j in (i+1)..tokens.len() {
                assert_ne!(tokens[i], tokens[j]);
            }
        }
    }
}
```

Don't forget to export in `mod.rs`:
```rust
pub use constants::{USDC_MINT, USDT_MINT, WSOL_MINT, JUP_MINT, PYTH_MINT, RAY_MINT};
```

## Verification

Run tests to verify:
```bash
cargo test chain::constants::
```

Check usage in codebase:
```bash
grep -r "USDC_MINT\|USDT_MINT\|WSOL_MINT" src/
```

## Next Steps

1. **Use in opportunity detection** - Filter for major token pairs
2. **Add to risk assessment** - Different risk levels per token
3. **Enhance reporting** - Display token symbols instead of addresses
4. **Add more constants** - As you discover other important tokens
5. **Create token utilities** - Helper functions using these constants

---

**Token constants are ready to use!** Import them wherever you need to check for USDC, USDT, or WSOL.
