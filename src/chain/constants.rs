// Feature: Define Well-Known Token Mint Addresses
// 
// Tasks (in order):
// 1. Define three new public constants of type `Pubkey` for the major stablecoins and Wrapped SOL.
// 2. Use the addresses found via Solscan:
//    - USDC_MINT: EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
//    - USDT_MINT: Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB
//    - WSOL_MINT: So11111111111111111111111111111111111111112

// DECISION: Use constants (Chosen) vs reading from config.
// Chosen: Constants are ideal for immutable, well-known program and token IDs, improving readability and performance.

// OPTIMIZE: Using `solana_program::pubkey!` macro for compile-time Pubkey creation - 
// this is more efficient than runtime parsing and catches invalid addresses at compile time.

use solana_program::pubkey::Pubkey;

/// USDC token mint address (6 decimals)
/// Official Circle USD Coin on Solana
pub const USDC_MINT: Pubkey = solana_program::pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

/// USDT token mint address (6 decimals)
/// Official Tether USD on Solana
pub const USDT_MINT: Pubkey = solana_program::pubkey!("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");

/// Wrapped SOL token mint address (9 decimals)
/// Native SOL wrapped as SPL token
pub const WSOL_MINT: Pubkey = solana_program::pubkey!("So11111111111111111111111111111111111111112");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usdc_mint_address() {
        let expected = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        assert_eq!(USDC_MINT.to_string(), expected);
    }

    #[test]
    fn test_usdt_mint_address() {
        let expected = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
        assert_eq!(USDT_MINT.to_string(), expected);
    }

    #[test]
    fn test_wsol_mint_address() {
        let expected = "So11111111111111111111111111111111111111112";
        assert_eq!(WSOL_MINT.to_string(), expected);
    }

    #[test]
    fn test_addresses_are_different() {
        assert_ne!(USDC_MINT, USDT_MINT);
        assert_ne!(USDC_MINT, WSOL_MINT);
        assert_ne!(USDT_MINT, WSOL_MINT);
    }
}
