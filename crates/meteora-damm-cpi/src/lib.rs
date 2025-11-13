#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

// Program ID for Meteora DAMM
declare_id!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"); // Placeholder - replace with actual program ID

/// Generate CPI module from IDL
/// Note: In production, you would use anchor-gen crate to generate this from the IDL
/// For now, we'll manually define the structs and instructions based on the IDL

// ============================================================================
// Account Structs (Generated from IDL)
// ============================================================================

#[account]
#[derive(Debug)]
pub struct Pool {
    pub curve_type: u8,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_vault: Pubkey,
    pub token_b_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub trade_fee_numerator: u64,
    pub trade_fee_denominator: u64,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
    pub lp_supply: u64,
    pub bump: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub enum CurveType {
    ConstantProduct = 0,
    Stable = 1,
    Weighted = 2,
}

// ============================================================================
// Instruction Contexts (Generated from IDL)
// ============================================================================

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user_source_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_destination_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_source_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_destination_vault: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub token_a_user_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_b_user_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_a_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_b_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub lp_mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub user_lp_token: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub token_a_user_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_b_user_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_a_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_b_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub lp_mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub user_lp_token: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

// ============================================================================
// Helper Methods (Custom implementations on generated structs)
// ============================================================================

impl Pool {
    /// Calculate expected output amount for a swap using constant product formula (x * y = k)
    /// This is a simplified calculation - actual implementation depends on curve_type
    pub fn calculate_swap_output(
        &self,
        amount_in: u64,
        source_is_token_a: bool,
    ) -> Result<u64> {
        let (reserve_in, reserve_out) = if source_is_token_a {
            (self.token_a_amount, self.token_b_amount)
        } else {
            (self.token_b_amount, self.token_a_amount)
        };

        require!(reserve_in > 0 && reserve_out > 0, ErrorCode::InsufficientLiquidity);
        require!(amount_in > 0, ErrorCode::InvalidAmount);

        match self.curve_type {
            0 => self.calculate_constant_product_output(amount_in, reserve_in, reserve_out),
            1 => self.calculate_stable_swap_output(amount_in, reserve_in, reserve_out),
            _ => Err(ErrorCode::InvalidCurveType.into()),
        }
    }

    /// Constant product AMM calculation (Uniswap v2 style)
    fn calculate_constant_product_output(
        &self,
        amount_in: u64,
        reserve_in: u64,
        reserve_out: u64,
    ) -> Result<u64> {
        // Apply fee: amount_in_with_fee = amount_in * (1 - fee)
        let fee_amount = (amount_in as u128)
            .checked_mul(self.trade_fee_numerator as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(self.trade_fee_denominator as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        let amount_in_with_fee = amount_in.checked_sub(fee_amount as u64)
            .ok_or(ErrorCode::MathOverflow)?;

        // Calculate output: amount_out = (amount_in_with_fee * reserve_out) / (reserve_in + amount_in_with_fee)
        let numerator = (amount_in_with_fee as u128)
            .checked_mul(reserve_out as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        let denominator = (reserve_in as u128)
            .checked_add(amount_in_with_fee as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        let amount_out = numerator
            .checked_div(denominator)
            .ok_or(ErrorCode::MathOverflow)?;

        Ok(amount_out as u64)
    }

    /// Stable swap calculation (StableSwap invariant)
    /// Simplified - production version would use full StableSwap curve
    fn calculate_stable_swap_output(
        &self,
        amount_in: u64,
        reserve_in: u64,
        reserve_out: u64,
    ) -> Result<u64> {
        // For stable swaps, use a simplified approximation
        // Real implementation would use the StableSwap invariant formula
        
        let fee_amount = (amount_in as u128)
            .checked_mul(self.trade_fee_numerator as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(self.trade_fee_denominator as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        let amount_in_with_fee = amount_in.checked_sub(fee_amount as u64)
            .ok_or(ErrorCode::MathOverflow)?;

        // Simplified stable calculation (approximately 1:1 for small amounts)
        let slippage_factor = (amount_in_with_fee as u128)
            .checked_mul(reserve_in as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div((reserve_in as u128).checked_add(reserve_out as u128).ok_or(ErrorCode::MathOverflow)?)
            .ok_or(ErrorCode::MathOverflow)?;
        
        let amount_out = (amount_in_with_fee as u128)
            .checked_sub(slippage_factor)
            .ok_or(ErrorCode::MathOverflow)?;

        Ok(amount_out as u64)
    }

    /// Calculate the price of token A in terms of token B
    pub fn get_price_a_to_b(&self) -> Result<f64> {
        require!(self.token_b_amount > 0, ErrorCode::InsufficientLiquidity);
        Ok(self.token_a_amount as f64 / self.token_b_amount as f64)
    }

    /// Calculate the price of token B in terms of token A
    pub fn get_price_b_to_a(&self) -> Result<f64> {
        require!(self.token_a_amount > 0, ErrorCode::InsufficientLiquidity);
        Ok(self.token_b_amount as f64 / self.token_a_amount as f64)
    }

    /// Calculate LP tokens to mint for a given deposit
    pub fn calculate_lp_tokens_for_deposit(
        &self,
        token_a_amount: u64,
        token_b_amount: u64,
    ) -> Result<u64> {
        if self.lp_supply == 0 {
            // Initial deposit - use geometric mean
            let product = (token_a_amount as u128)
                .checked_mul(token_b_amount as u128)
                .ok_or(ErrorCode::MathOverflow)?;
            
            // Simple square root approximation
            let lp_amount = (product as f64).sqrt() as u64;
            Ok(lp_amount)
        } else {
            // Proportional deposit
            let lp_from_a = (token_a_amount as u128)
                .checked_mul(self.lp_supply as u128)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(self.token_a_amount as u128)
                .ok_or(ErrorCode::MathOverflow)?;
            
            let lp_from_b = (token_b_amount as u128)
                .checked_mul(self.lp_supply as u128)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(self.token_b_amount as u128)
                .ok_or(ErrorCode::MathOverflow)?;
            
            // Take minimum to maintain ratio
            Ok(lp_from_a.min(lp_from_b) as u64)
        }
    }

    /// Calculate token amounts to withdraw for given LP tokens
    pub fn calculate_tokens_for_lp(&self, lp_amount: u64) -> Result<(u64, u64)> {
        require!(self.lp_supply > 0, ErrorCode::InvalidShareAmount);
        
        let token_a_out = (lp_amount as u128)
            .checked_mul(self.token_a_amount as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(self.lp_supply as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        let token_b_out = (lp_amount as u128)
            .checked_mul(self.token_b_amount as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(self.lp_supply as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        Ok((token_a_out as u64, token_b_out as u64))
    }

    /// Get the effective fee in basis points
    pub fn get_fee_bps(&self) -> u64 {
        (self.trade_fee_numerator * 10000) / self.trade_fee_denominator
    }

    /// Check if pool has sufficient liquidity for a trade
    pub fn has_sufficient_liquidity(&self, amount_in: u64, source_is_token_a: bool) -> bool {
        let reserve_in = if source_is_token_a {
            self.token_a_amount
        } else {
            self.token_b_amount
        };
        
        // Ensure trade is less than 50% of reserves to prevent excessive slippage
        amount_in < reserve_in / 2
    }
}

// ============================================================================
// Error Codes
// ============================================================================

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid curve type")]
    InvalidCurveType,
    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Invalid fee parameters")]
    InvalidFeeParameters,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Invalid share amount")]
    InvalidShareAmount,
    #[msg("Math overflow")]
    MathOverflow,
}

// ============================================================================
// CPI Helper Functions
// ============================================================================

/// Helper to build swap CPI instruction data
pub fn build_swap_instruction_data(amount_in: u64, minimum_amount_out: u64) -> Vec<u8> {
    // Instruction discriminator (first 8 bytes of sha256("global:swap"))
    let mut data = vec![0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&minimum_amount_out.to_le_bytes());
    data
}

/// Helper to build add_liquidity CPI instruction data
pub fn build_add_liquidity_instruction_data(
    token_a_amount: u64,
    token_b_amount: u64,
    min_lp_amount: u64,
) -> Vec<u8> {
    // Instruction discriminator (first 8 bytes of sha256("global:add_liquidity"))
    let mut data = vec![0x18, 0x1e, 0xc8, 0x28, 0x05, 0x1c, 0x07, 0x77];
    data.extend_from_slice(&token_a_amount.to_le_bytes());
    data.extend_from_slice(&token_b_amount.to_le_bytes());
    data.extend_from_slice(&min_lp_amount.to_le_bytes());
    data
}

/// Helper to build remove_liquidity CPI instruction data
pub fn build_remove_liquidity_instruction_data(
    lp_amount: u64,
    min_token_a_amount: u64,
    min_token_b_amount: u64,
) -> Vec<u8> {
    // Instruction discriminator (first 8 bytes of sha256("global:remove_liquidity"))
    let mut data = vec![0x52, 0xaa, 0x7f, 0x52, 0x06, 0x81, 0x5e, 0x44];
    data.extend_from_slice(&lp_amount.to_le_bytes());
    data.extend_from_slice(&min_token_a_amount.to_le_bytes());
    data.extend_from_slice(&min_token_b_amount.to_le_bytes());
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_product_calculation() {
        let pool = Pool {
            curve_type: 0,
            token_a_mint: Pubkey::default(),
            token_b_mint: Pubkey::default(),
            token_a_vault: Pubkey::default(),
            token_b_vault: Pubkey::default(),
            lp_mint: Pubkey::default(),
            trade_fee_numerator: 25,
            trade_fee_denominator: 10000,
            token_a_amount: 1_000_000,
            token_b_amount: 1_000_000,
            lp_supply: 1_000_000,
            bump: 255,
        };

        let amount_in = 1000;
        let result = pool.calculate_swap_output(amount_in, true).unwrap();
        
        // Output should be slightly less than 1000 due to fees and slippage
        assert!(result < amount_in);
        assert!(result > 0);
    }

    #[test]
    fn test_price_calculation() {
        let pool = Pool {
            curve_type: 0,
            token_a_mint: Pubkey::default(),
            token_b_mint: Pubkey::default(),
            token_a_vault: Pubkey::default(),
            token_b_vault: Pubkey::default(),
            lp_mint: Pubkey::default(),
            trade_fee_numerator: 25,
            trade_fee_denominator: 10000,
            token_a_amount: 2_000_000,
            token_b_amount: 1_000_000,
            lp_supply: 1_000_000,
            bump: 255,
        };

        let price_a_to_b = pool.get_price_a_to_b().unwrap();
        assert_eq!(price_a_to_b, 2.0);

        let price_b_to_a = pool.get_price_b_to_a().unwrap();
        assert_eq!(price_b_to_a, 0.5);
    }

    #[test]
    fn test_fee_calculation() {
        let pool = Pool {
            curve_type: 0,
            token_a_mint: Pubkey::default(),
            token_b_mint: Pubkey::default(),
            token_a_vault: Pubkey::default(),
            token_b_vault: Pubkey::default(),
            lp_mint: Pubkey::default(),
            trade_fee_numerator: 25,
            trade_fee_denominator: 10000,
            token_a_amount: 1_000_000,
            token_b_amount: 1_000_000,
            lp_supply: 1_000_000,
            bump: 255,
        };

        let fee_bps = pool.get_fee_bps();
        assert_eq!(fee_bps, 25); // 0.25%
    }
}
