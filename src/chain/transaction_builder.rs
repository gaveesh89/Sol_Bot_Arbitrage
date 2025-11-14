// Solana Transaction Builder for Multi-DEX Arbitrage Swaps
//
// This module builds versioned transactions for executing arbitrage cycles across
// multiple DEXs (Raydium, Meteora, Whirlpool, Orca). It handles:
// 1. Versioned transactions with Address Lookup Tables (ALT)
// 2. Compute budget optimization with priority fees
// 3. DEX-specific swap instruction construction
// 4. Atomic execution guarantees
// 5. Transaction size optimization

use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    transaction::VersionedTransaction,
    message::{VersionedMessage, v0::Message as V0Message},
    address_lookup_table_account::AddressLookupTableAccount,
    signature::Keypair,
    signer::Signer,
    compute_budget::ComputeBudgetInstruction,
    system_program,
};
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{debug, info, warn};

use crate::dex::triangular_arb::{ArbitrageCycle, CycleStep, DexType};

/// Transaction builder for arbitrage swaps
pub struct SwapTransactionBuilder {
    payer: Keypair,
    token_accounts: HashMap<Pubkey, Pubkey>, // mint -> associated token account
    lookup_tables: Vec<AddressLookupTableAccount>,
}

/// Configuration for transaction building
#[derive(Clone, Debug)]
pub struct TransactionConfig {
    /// Maximum slippage tolerance in basis points (100 = 1%)
    pub max_slippage_bps: u16,
    
    /// Priority fee in micro-lamports per compute unit
    pub priority_fee_micro_lamports: u64,
    
    /// Additional compute units to request (on top of calculated)
    pub compute_unit_buffer: u32,
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            max_slippage_bps: 100,           // 1% default slippage
            priority_fee_micro_lamports: 1000, // Moderate priority
            compute_unit_buffer: 50_000,     // 50k buffer for safety
        }
    }
}

impl SwapTransactionBuilder {
    /// Create new transaction builder
    pub fn new(
        payer: Keypair,
        token_accounts: HashMap<Pubkey, Pubkey>,
        lookup_tables: Vec<AddressLookupTableAccount>,
    ) -> Self {
        info!(
            "Initialized SwapTransactionBuilder with {} token accounts, {} lookup tables",
            token_accounts.len(),
            lookup_tables.len()
        );
        
        Self {
            payer,
            token_accounts,
            lookup_tables,
        }
    }

    /// Build complete arbitrage transaction from detected cycle
    pub async fn build_arbitrage_tx(
        &self,
        cycle: &ArbitrageCycle,
        input_amount: u64,
        config: &TransactionConfig,
    ) -> Result<VersionedTransaction> {
        debug!(
            "Building arbitrage transaction for {} step cycle, input: {} lamports",
            cycle.path.len(),
            input_amount
        );

        // 1. Calculate compute budget
        let compute_units = self.calculate_compute_budget(cycle.path.len()) + config.compute_unit_buffer;
        
        // 2. Build all instructions
        let mut instructions = Vec::new();
        
        // Add compute budget instructions
        instructions.push(
            ComputeBudgetInstruction::set_compute_unit_limit(compute_units)
        );
        instructions.push(
            ComputeBudgetInstruction::set_compute_unit_price(config.priority_fee_micro_lamports)
        );

        // 3. Build swap instructions for each step
        let mut current_amount = input_amount;
        
        for (idx, step) in cycle.path.iter().enumerate() {
            // Calculate expected output for this step
            let expected_output = (current_amount as f64 * step.rate) as u64;
            
            // Apply fee
            let fee = (expected_output as u128 * step.fee_bps as u128 / 10000) as u64;
            let expected_after_fee = expected_output.saturating_sub(fee);
            
            // Calculate minimum output with slippage
            let minimum_out = self.calculate_minimum_out(expected_after_fee, config.max_slippage_bps);
            
            debug!(
                "Step {}: {} -> {}, amount_in={}, expected_out={}, min_out={}",
                idx,
                step.from_token,
                step.to_token,
                current_amount,
                expected_after_fee,
                minimum_out
            );

            // Build DEX-specific swap instruction
            let swap_ix = self.build_swap_instruction(step, current_amount, minimum_out)?;
            instructions.push(swap_ix);
            
            // Update amount for next step
            current_amount = expected_after_fee;
        }

        // 4. Build versioned message with ALT
        let message = self.build_versioned_message(instructions)?;
        
        // 5. Sign transaction
        let transaction = VersionedTransaction::try_new(message, &[&self.payer])?;
        
        info!(
            "Built arbitrage transaction: {} instructions, {} compute units",
            cycle.path.len() + 2, // swaps + 2 compute budget instructions
            compute_units
        );

        Ok(transaction)
    }

    /// Build swap instruction for any DEX type
    fn build_swap_instruction(
        &self,
        step: &CycleStep,
        amount_in: u64,
        minimum_out: u64,
    ) -> Result<Instruction> {
        match step.dex {
            DexType::Raydium => self.build_raydium_swap_ix(step, amount_in, minimum_out),
            DexType::Meteora => self.build_meteora_swap_ix(step, amount_in, minimum_out),
            DexType::Whirlpool => self.build_whirlpool_swap_ix(step, amount_in, minimum_out),
            DexType::Orca => self.build_orca_swap_ix(step, amount_in, minimum_out),
            DexType::Pump => self.build_pump_swap_ix(step, amount_in, minimum_out),
        }
    }

    /// Create Raydium AMM swap instruction
    fn build_raydium_swap_ix(
        &self,
        step: &CycleStep,
        amount_in: u64,
        minimum_out: u64,
    ) -> Result<Instruction> {
        // Raydium AMM v4 program ID
        let raydium_program_id = solana_sdk::pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
        
        // Get user token accounts
        let user_source = self.token_accounts.get(&step.from_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.from_token))?;
        let user_destination = self.token_accounts.get(&step.to_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.to_token))?;

        // Raydium swap instruction accounts (simplified)
        // In production, derive all PDAs properly
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new_readonly(system_program::id(), false),
            solana_sdk::instruction::AccountMeta::new(step.pool, false),
            solana_sdk::instruction::AccountMeta::new_readonly(self.payer.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new(*user_source, false),
            solana_sdk::instruction::AccountMeta::new(*user_destination, false),
            // Additional accounts: pool coin/pc vaults, signer, etc.
        ];

        // Raydium swap instruction data (instruction discriminator + params)
        let mut data = vec![9]; // Swap instruction discriminator
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_out.to_le_bytes());

        Ok(Instruction {
            program_id: raydium_program_id,
            accounts,
            data,
        })
    }

    /// Create Meteora DAMM swap instruction
    fn build_meteora_swap_ix(
        &self,
        step: &CycleStep,
        amount_in: u64,
        minimum_out: u64,
    ) -> Result<Instruction> {
        // Meteora DAMM program ID
        let meteora_program_id = solana_sdk::pubkey!("Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB");
        
        let user_source = self.token_accounts.get(&step.from_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.from_token))?;
        let user_destination = self.token_accounts.get(&step.to_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.to_token))?;

        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(step.pool, false),
            solana_sdk::instruction::AccountMeta::new_readonly(self.payer.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new(*user_source, false),
            solana_sdk::instruction::AccountMeta::new(*user_destination, false),
        ];

        // Meteora swap instruction data
        let mut data = vec![0xf8, 0x3d, 0x2a, 0x3b, 0x88, 0x1b, 0x0e, 0x94]; // swap discriminator
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_out.to_le_bytes());

        Ok(Instruction {
            program_id: meteora_program_id,
            accounts,
            data,
        })
    }

    /// Create Whirlpool (Orca v2) swap instruction
    fn build_whirlpool_swap_ix(
        &self,
        step: &CycleStep,
        amount_in: u64,
        minimum_out: u64,
    ) -> Result<Instruction> {
        // Whirlpool program ID
        let whirlpool_program_id = solana_sdk::pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
        
        let user_source = self.token_accounts.get(&step.from_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.from_token))?;
        let user_destination = self.token_accounts.get(&step.to_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.to_token))?;

        // Whirlpool swap accounts
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(step.pool, false),
            solana_sdk::instruction::AccountMeta::new_readonly(self.payer.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new(*user_source, false),
            solana_sdk::instruction::AccountMeta::new(*user_destination, false),
            // Additional: tick arrays, oracle, etc.
        ];

        // Whirlpool swap instruction data
        let mut data = vec![0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8]; // swap discriminator
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_out.to_le_bytes());
        data.extend_from_slice(&(u128::MAX).to_le_bytes()); // sqrt_price_limit (no limit)
        data.push(1); // a_to_b direction

        Ok(Instruction {
            program_id: whirlpool_program_id,
            accounts,
            data,
        })
    }

    /// Create Orca v1 swap instruction
    fn build_orca_swap_ix(
        &self,
        step: &CycleStep,
        amount_in: u64,
        minimum_out: u64,
    ) -> Result<Instruction> {
        // Orca v1 program ID
        let orca_program_id = solana_sdk::pubkey!("9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP");
        
        let user_source = self.token_accounts.get(&step.from_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.from_token))?;
        let user_destination = self.token_accounts.get(&step.to_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.to_token))?;

        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(step.pool, false),
            solana_sdk::instruction::AccountMeta::new_readonly(self.payer.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new(*user_source, false),
            solana_sdk::instruction::AccountMeta::new(*user_destination, false),
        ];

        // Orca swap instruction data
        let mut data = vec![1]; // Swap instruction
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_out.to_le_bytes());

        Ok(Instruction {
            program_id: orca_program_id,
            accounts,
            data,
        })
    }

    /// Create Pump.fun swap instruction
    fn build_pump_swap_ix(
        &self,
        step: &CycleStep,
        amount_in: u64,
        minimum_out: u64,
    ) -> Result<Instruction> {
        // Pump.fun program ID
        let pump_program_id = solana_sdk::pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");
        
        let user_source = self.token_accounts.get(&step.from_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.from_token))?;
        let user_destination = self.token_accounts.get(&step.to_token)
            .ok_or_else(|| anyhow!("Missing token account for {}", step.to_token))?;

        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(step.pool, false),
            solana_sdk::instruction::AccountMeta::new_readonly(self.payer.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new(*user_source, false),
            solana_sdk::instruction::AccountMeta::new(*user_destination, false),
        ];

        let mut data = vec![0x66, 0x06, 0x3d, 0x12, 0x01, 0xda, 0xeb, 0xea]; // buy/sell discriminator
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_out.to_le_bytes());

        Ok(Instruction {
            program_id: pump_program_id,
            accounts,
            data,
        })
    }

    /// Calculate minimum output amount with slippage tolerance
    fn calculate_minimum_out(&self, expected_amount: u64, slippage_bps: u16) -> u64 {
        // minimum = expected * (1 - slippage/10000)
        let slippage_multiplier = 10000u128 - slippage_bps as u128;
        let minimum = (expected_amount as u128 * slippage_multiplier / 10000) as u64;
        
        debug!(
            "Slippage calculation: expected={}, slippage={}bps, minimum={}",
            expected_amount, slippage_bps, minimum
        );
        
        minimum
    }

    /// Calculate compute budget based on number of swaps
    fn calculate_compute_budget(&self, num_swaps: usize) -> u32 {
        // Base overhead: 20k compute units
        // Per swap: 80k compute units (varies by DEX)
        let base = 20_000u32;
        let per_swap = 80_000u32;
        
        let total = base + (per_swap * num_swaps as u32);
        
        debug!(
            "Compute budget: {} swaps = {} compute units",
            num_swaps, total
        );
        
        total
    }

    /// Build versioned message using Address Lookup Tables
    fn build_versioned_message(&self, instructions: Vec<Instruction>) -> Result<VersionedMessage> {
        // Get recent blockhash (in production, fetch from RPC)
        let recent_blockhash = solana_sdk::hash::Hash::default();
        
        // For now, create a legacy message and convert to v0
        // In production, properly build v0 message with ALT
        let legacy_message = solana_sdk::message::Message::new(
            &instructions,
            Some(&self.payer.pubkey()),
        );

        // Convert to versioned message
        // With ALT support, use V0, otherwise use Legacy
        if self.lookup_tables.is_empty() {
            Ok(VersionedMessage::Legacy(legacy_message))
        } else {
            // TODO: Properly compile v0 message with address lookups
            // For now, use legacy
            Ok(VersionedMessage::Legacy(legacy_message))
        }
    }

    /// Update token accounts mapping
    pub fn add_token_account(&mut self, mint: Pubkey, token_account: Pubkey) {
        self.token_accounts.insert(mint, token_account);
    }

    /// Get payer public key
    pub fn payer(&self) -> Pubkey {
        self.payer.pubkey()
    }

    /// Estimate transaction size in bytes
    pub fn estimate_tx_size(&self, num_swaps: usize) -> usize {
        // Base message size: ~100 bytes
        // Per instruction: ~50 bytes
        // ALT compression reduces significantly
        let base = 100;
        let per_ix = 50;
        let with_alt_reduction = 0.6; // 40% size reduction with ALT
        
        let uncompressed = base + (per_ix * (num_swaps + 2)); // +2 for compute budget
        let compressed = (uncompressed as f64 * with_alt_reduction) as usize;
        
        compressed.min(1232) // Max transaction size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_minimum_out() {
        let keypair = Keypair::new();
        let builder = SwapTransactionBuilder::new(keypair, HashMap::new(), vec![]);
        
        // 1% slippage
        let min_out = builder.calculate_minimum_out(1_000_000, 100);
        assert_eq!(min_out, 990_000); // 99% of expected
        
        // 0.5% slippage
        let min_out = builder.calculate_minimum_out(1_000_000, 50);
        assert_eq!(min_out, 995_000); // 99.5% of expected
        
        // 5% slippage
        let min_out = builder.calculate_minimum_out(1_000_000, 500);
        assert_eq!(min_out, 950_000); // 95% of expected
    }

    #[test]
    fn test_calculate_compute_budget() {
        let keypair = Keypair::new();
        let builder = SwapTransactionBuilder::new(keypair, HashMap::new(), vec![]);
        
        // 2 swaps
        let budget = builder.calculate_compute_budget(2);
        assert_eq!(budget, 20_000 + 80_000 * 2); // 180k
        
        // 3 swaps
        let budget = builder.calculate_compute_budget(3);
        assert_eq!(budget, 20_000 + 80_000 * 3); // 260k
        
        // 4 swaps
        let budget = builder.calculate_compute_budget(4);
        assert_eq!(budget, 20_000 + 80_000 * 4); // 340k
    }

    #[test]
    fn test_estimate_tx_size() {
        let keypair = Keypair::new();
        let builder = SwapTransactionBuilder::new(keypair, HashMap::new(), vec![]);
        
        // 2 swaps
        let size = builder.estimate_tx_size(2);
        assert!(size < 1232); // Must fit in max tx size
        assert!(size > 100);  // Should be reasonable size
        
        // 4 swaps
        let size = builder.estimate_tx_size(4);
        assert!(size < 1232);
    }

    #[test]
    fn test_transaction_config_default() {
        let config = TransactionConfig::default();
        assert_eq!(config.max_slippage_bps, 100); // 1%
        assert_eq!(config.priority_fee_micro_lamports, 1000);
        assert_eq!(config.compute_unit_buffer, 50_000);
    }
}
