#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

// Program ID for Meteora Vault
declare_id!("24Uqj9JCLxUeoC3hGfh5W3s9FM9uCHDS2SG3LYwBpyTi"); // Placeholder - replace with actual program ID

/// Generate CPI module from IDL
/// Note: In production, you would use anchor-gen crate to generate this from the IDL
/// For now, we'll manually define the structs and instructions based on the IDL

// ============================================================================
// Account Structs (Generated from IDL)
// ============================================================================

#[derive(Debug, Clone, Copy, AnchorSerialize, AnchorDeserialize)]
pub struct LockedProfitTracker {
    pub last_report: i64,
    pub last_locked_profit: u64,
    pub locked_profit: u64,
}

#[account]
#[derive(Debug)]
pub struct Vault {
    pub token_mint: Pubkey,
    pub token_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub total_assets: u64,
    pub total_shares: u64,
    pub locked_profit_tracker: LockedProfitTracker,
    pub locked_profit_degradation: u64, // Per-second degradation rate
    pub strategy: Option<Pubkey>,
    pub last_harvest_timestamp: i64,
    pub bump: u8,
}

// ============================================================================
// Instruction Contexts (Generated from IDL)
// ============================================================================

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub lp_mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub user_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_lp: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub lp_mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub user_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_lp: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Harvest<'info> {
    #[account(mut)]
    pub vault: Account<'info, Vault>,
    /// CHECK: Strategy account
    #[account(mut)]
    pub strategy: UncheckedAccount<'info>,
    #[account(mut)]
    pub token_vault: Account<'info, TokenAccount>,
    pub harvester: Signer<'info>,
}

#[derive(Accounts)]
pub struct Compound<'info> {
    #[account(mut)]
    pub vault: Account<'info, Vault>,
    /// CHECK: Strategy account
    #[account(mut)]
    pub strategy: UncheckedAccount<'info>,
    #[account(mut)]
    pub token_vault: Account<'info, TokenAccount>,
}

// ============================================================================
// Helper Methods (Custom implementations on generated structs)
// ============================================================================

impl Vault {
    /// Calculate the unlocked amount of assets (total assets minus locked profit)
    pub fn get_unlocked_amount(&self, current_timestamp: i64) -> Result<u64> {
        let locked_profit = self.calculate_locked_profit(current_timestamp)?;
        
        Ok(self.total_assets.saturating_sub(locked_profit))
    }

    /// Calculate the amount of shares to mint for a given deposit
    pub fn convert_to_shares(&self, assets: u64, current_timestamp: i64) -> Result<u64> {
        let unlocked_assets = self.get_unlocked_amount(current_timestamp)?;
        
        if self.total_shares == 0 || unlocked_assets == 0 {
            // Initial deposit - 1:1 ratio
            Ok(assets)
        } else {
            // Proportional shares based on unlocked assets
            let shares = (assets as u128)
                .checked_mul(self.total_shares as u128)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(unlocked_assets as u128)
                .ok_or(ErrorCode::MathOverflow)?;
            
            Ok(shares as u64)
        }
    }

    /// Calculate the amount of assets to withdraw for given shares
    pub fn convert_to_assets(&self, shares: u64, current_timestamp: i64) -> Result<u64> {
        require!(self.total_shares > 0, ErrorCode::InvalidShareAmount);
        
        let unlocked_assets = self.get_unlocked_amount(current_timestamp)?;
        
        let assets = (shares as u128)
            .checked_mul(unlocked_assets as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(self.total_shares as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        Ok(assets as u64)
    }

    /// Get the current share price (assets per share)
    pub fn get_share_price(&self, current_timestamp: i64) -> Result<f64> {
        if self.total_shares == 0 {
            return Ok(1.0);
        }
        
        let unlocked_assets = self.get_unlocked_amount(current_timestamp)?;
        Ok(unlocked_assets as f64 / self.total_shares as f64)
    }

    /// Calculate the current APY based on recent performance
    /// This is a simplified calculation - production version would use historical data
    pub fn estimate_apy(&self, current_timestamp: i64) -> Result<f64> {
        if self.last_harvest_timestamp == 0 {
            return Ok(0.0);
        }

        let time_elapsed = current_timestamp.saturating_sub(self.last_harvest_timestamp);
        if time_elapsed == 0 {
            return Ok(0.0);
        }

        let locked_profit = self.locked_profit_tracker.locked_profit;
        let total_value = self.total_assets;
        
        if total_value == 0 {
            return Ok(0.0);
        }

        // Calculate annualized return
        let profit_rate = locked_profit as f64 / total_value as f64;
        let seconds_per_year = 365.25 * 24.0 * 60.0 * 60.0;
        let annualization_factor = seconds_per_year / time_elapsed as f64;
        
        Ok(profit_rate * annualization_factor * 100.0) // Return as percentage
    }

    /// Check if vault has sufficient liquidity for withdrawal
    pub fn has_sufficient_liquidity(&self, shares: u64, current_timestamp: i64) -> Result<bool> {
        let assets_needed = self.convert_to_assets(shares, current_timestamp)?;
        Ok(assets_needed <= self.total_assets)
    }

    /// Get the maximum withdrawable amount for current liquidity
    pub fn get_max_withdrawable_shares(&self, current_timestamp: i64) -> Result<u64> {
        let unlocked_assets = self.get_unlocked_amount(current_timestamp)?;
        
        if self.total_shares == 0 || unlocked_assets == 0 {
            return Ok(0);
        }

        let max_shares = (unlocked_assets as u128)
            .checked_mul(self.total_shares as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(self.total_assets as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        Ok(max_shares as u64)
    }
}

impl LockedProfitTracker {
    /// Calculate the currently locked profit based on degradation over time
    pub fn calculate_locked_profit(&self, current_timestamp: i64, degradation_per_second: u64) -> Result<u64> {
        // If current time is before/equal to last_report, no degradation yet
        if current_timestamp <= self.last_report {
            return Ok(self.locked_profit);
        }

        let time_elapsed = current_timestamp.saturating_sub(self.last_report) as u64;
        
        // Calculate degradation: locked_profit decreases linearly over time
        let degradation = time_elapsed
            .checked_mul(degradation_per_second)
            .ok_or(ErrorCode::MathOverflow)?;
        
        let remaining_locked = self.locked_profit.saturating_sub(degradation);
        
        Ok(remaining_locked)
    }

    /// Check if profit is fully unlocked
    pub fn is_fully_unlocked(&self, current_timestamp: i64, degradation_per_second: u64) -> Result<bool> {
        let locked = self.calculate_locked_profit(current_timestamp, degradation_per_second)?;
        Ok(locked == 0)
    }

    /// Calculate the time remaining until profits are fully unlocked
    pub fn time_until_fully_unlocked(&self, current_timestamp: i64, degradation_per_second: u64) -> Result<i64> {
        if degradation_per_second == 0 {
            return Ok(i64::MAX);
        }

        let locked = self.calculate_locked_profit(current_timestamp, degradation_per_second)?;
        
        if locked == 0 {
            return Ok(0);
        }

        let time_remaining = (locked as u128)
            .checked_div(degradation_per_second as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        Ok(time_remaining as i64)
    }
}

// Implement helper on Vault to use LockedProfitTracker
impl Vault {
    /// Calculate the currently locked profit
    pub fn calculate_locked_profit(&self, current_timestamp: i64) -> Result<u64> {
        self.locked_profit_tracker.calculate_locked_profit(
            current_timestamp,
            self.locked_profit_degradation,
        )
    }

    /// Check if all profits are unlocked
    pub fn is_profit_fully_unlocked(&self, current_timestamp: i64) -> Result<bool> {
        self.locked_profit_tracker.is_fully_unlocked(
            current_timestamp,
            self.locked_profit_degradation,
        )
    }

    /// Get time until profits are fully unlocked
    pub fn time_until_unlocked(&self, current_timestamp: i64) -> Result<i64> {
        self.locked_profit_tracker.time_until_fully_unlocked(
            current_timestamp,
            self.locked_profit_degradation,
        )
    }
}

// ============================================================================
// Error Codes
// ============================================================================

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("Invalid share amount")]
    InvalidShareAmount,
    #[msg("Vault is locked")]
    VaultLocked,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Invalid timestamp")]
    InvalidTimestamp,
}

// ============================================================================
// CPI Helper Functions
// ============================================================================

/// Helper to build deposit CPI instruction data
pub fn build_deposit_instruction_data(amount: u64) -> Vec<u8> {
    // Instruction discriminator (first 8 bytes of sha256("global:deposit"))
    let mut data = vec![0xf2, 0x23, 0xc6, 0x89, 0x50, 0x7a, 0x3b, 0x2a];
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

/// Helper to build withdraw CPI instruction data
pub fn build_withdraw_instruction_data(shares: u64) -> Vec<u8> {
    // Instruction discriminator (first 8 bytes of sha256("global:withdraw"))
    let mut data = vec![0xb7, 0x12, 0x46, 0x9c, 0x94, 0x6d, 0xa1, 0x22];
    data.extend_from_slice(&shares.to_le_bytes());
    data
}

/// Helper to build harvest CPI instruction data
pub fn build_harvest_instruction_data() -> Vec<u8> {
    // Instruction discriminator (first 8 bytes of sha256("global:harvest"))
    vec![0x84, 0x3e, 0x34, 0xf8, 0x47, 0xa6, 0xfa, 0x8f]
}

/// Helper to build compound CPI instruction data
pub fn build_compound_instruction_data() -> Vec<u8> {
    // Instruction discriminator (first 8 bytes of sha256("global:compound"))
    vec![0x1a, 0x5e, 0x87, 0x24, 0xc5, 0x9d, 0x3e, 0x71]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_vault() -> Vault {
        Vault {
            token_mint: Pubkey::default(),
            token_vault: Pubkey::default(),
            lp_mint: Pubkey::default(),
            total_assets: 1_000_000,
            total_shares: 1_000_000,
            locked_profit_tracker: LockedProfitTracker {
                last_report: 0,
                last_locked_profit: 0,
                locked_profit: 100_000,
            },
            locked_profit_degradation: 10, // 10 units per second
            strategy: None,
            last_harvest_timestamp: 0,
            bump: 255,
        }
    }

    #[test]
    fn test_unlocked_amount() {
        let mut vault = create_test_vault();
        vault.locked_profit_tracker.last_report = 0; // Set baseline at time 0
        
        // At time 0, no degradation yet
        let unlocked_at_0 = vault.get_unlocked_amount(0).unwrap();
        assert_eq!(unlocked_at_0, 900_000); // 1M - 100k locked
        
        // After 5000 seconds with 10 degradation/sec
        let unlocked_at_5000 = vault.get_unlocked_amount(5000).unwrap();
        // Initial locked: 100_000, degraded: 5000 * 10 = 50_000
        // Remaining locked: 50_000, so unlocked = 1_000_000 - 50_000 = 950_000
        assert_eq!(unlocked_at_5000, 950_000);
    }

    #[test]
    fn test_convert_to_shares() {
        let vault = create_test_vault();
        let current_time = 0;
        
        let shares = vault.convert_to_shares(100_000, current_time).unwrap();
        
        // With 1M total assets, 100k locked, 900k unlocked, and 1M shares
        // 100k assets should get: 100_000 * 1_000_000 / 900_000 = ~111,111 shares
        assert!(shares > 100_000);
    }

    #[test]
    fn test_convert_to_assets() {
        let vault = create_test_vault();
        let current_time = 0;
        
        let assets = vault.convert_to_assets(100_000, current_time).unwrap();
        
        // With 100k shares out of 1M total, and 900k unlocked assets
        // Should get: 100_000 * 900_000 / 1_000_000 = 90_000 assets
        assert_eq!(assets, 90_000);
    }

    #[test]
    fn test_share_price() {
        let vault = create_test_vault();
        let current_time = 0;
        
        let price = vault.get_share_price(current_time).unwrap();
        
        // 900k unlocked / 1M shares = 0.9
        assert_eq!(price, 0.9);
    }

    #[test]
    fn test_locked_profit_degradation() {
        let tracker = LockedProfitTracker {
            last_report: 0, // Time 0 baseline
            last_locked_profit: 0,
            locked_profit: 100_000,
        };
        
        // At time 0, no elapsed time, so locked profit remains
        let locked_at_0 = tracker.calculate_locked_profit(0, 10).unwrap();
        assert_eq!(locked_at_0, 100_000);
        
        // At time 5000, elapsed = 5000 - 0 = 5000 seconds
        // Degradation = 5000 * 10 = 50_000
        // Remaining = 100_000 - 50_000 = 50_000
        let locked_at_5000 = tracker.calculate_locked_profit(5000, 10).unwrap();
        assert_eq!(locked_at_5000, 50_000);
        
        // At time 10000, elapsed = 10000 seconds
        // Degradation = 10000 * 10 = 100_000
        // Remaining = 100_000 - 100_000 = 0
        let locked_at_10000 = tracker.calculate_locked_profit(10000, 10).unwrap();
        assert_eq!(locked_at_10000, 0);
    }

    #[test]
    fn test_time_until_unlocked() {
        let vault = create_test_vault();
        let current_time = 0;
        
        let time_remaining = vault.time_until_unlocked(current_time).unwrap();
        
        // 100_000 locked / 10 per second = 10_000 seconds
        assert_eq!(time_remaining, 10_000);
    }
}
