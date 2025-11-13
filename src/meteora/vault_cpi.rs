use anyhow::{Context, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use std::sync::Arc;
use tracing::info;

/// Meteora Vault Client for CPI interactions
pub struct MeteoraVaultClient {
    rpc_client: Arc<RpcClient>,
    program_id: Pubkey,
    payer: Arc<Keypair>,
}

impl MeteoraVaultClient {
    pub fn new(rpc_client: Arc<RpcClient>, program_id: Pubkey, payer: Arc<Keypair>) -> Self {
        info!("Initialized Meteora Vault Client with program ID: {}", program_id);
        Self {
            rpc_client,
            program_id,
            payer,
        }
    }

    /// Deposit tokens into Meteora Vault
    pub async fn deposit(
        &self,
        vault: &Pubkey,
        user_token_account: &Pubkey,
        amount: u64,
    ) -> Result<String> {
        info!(
            "Depositing {} tokens to Meteora Vault: {}",
            amount, vault
        );

        let instruction = self.build_deposit_instruction(vault, user_token_account, amount)?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&*self.payer],
            recent_blockhash,
        );

        let signature = self
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .await
            .context("Failed to deposit to Meteora Vault")?;

        info!("Vault deposit executed successfully: {}", signature);
        Ok(signature.to_string())
    }

    /// Withdraw tokens from Meteora Vault
    pub async fn withdraw(
        &self,
        vault: &Pubkey,
        user_token_account: &Pubkey,
        amount: u64,
    ) -> Result<String> {
        info!(
            "Withdrawing {} tokens from Meteora Vault: {}",
            amount, vault
        );

        let instruction = self.build_withdraw_instruction(vault, user_token_account, amount)?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&*self.payer],
            recent_blockhash,
        );

        let signature = self
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .await
            .context("Failed to withdraw from Meteora Vault")?;

        info!("Vault withdrawal executed successfully: {}", signature);
        Ok(signature.to_string())
    }

    /// Harvest rewards from Meteora Vault
    pub async fn harvest_rewards(
        &self,
        vault: &Pubkey,
        user_reward_account: &Pubkey,
    ) -> Result<String> {
        info!("Harvesting rewards from Meteora Vault: {}", vault);

        let instruction = self.build_harvest_instruction(vault, user_reward_account)?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&*self.payer],
            recent_blockhash,
        );

        let signature = self
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .await
            .context("Failed to harvest rewards from Meteora Vault")?;

        info!("Rewards harvested successfully: {}", signature);
        Ok(signature.to_string())
    }

    /// Compound rewards in Meteora Vault (auto-reinvest)
    pub async fn compound(&self, vault: &Pubkey) -> Result<String> {
        info!("Compounding rewards in Meteora Vault: {}", vault);

        let instruction = self.build_compound_instruction(vault)?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&*self.payer],
            recent_blockhash,
        );

        let signature = self
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .await
            .context("Failed to compound rewards in Meteora Vault")?;

        info!("Rewards compounded successfully: {}", signature);
        Ok(signature.to_string())
    }

    // Private helper methods to build instructions

    fn build_deposit_instruction(
        &self,
        vault: &Pubkey,
        user_token_account: &Pubkey,
        amount: u64,
    ) -> Result<Instruction> {
        // Placeholder implementation
        // You need to implement the actual instruction building based on Meteora Vault's IDL
        
        let accounts = vec![
            AccountMeta::new(*vault, false),
            AccountMeta::new(*user_token_account, false),
            AccountMeta::new(self.payer.pubkey(), true),
            // Add other required accounts (vault token account, mint, etc.)
        ];

        let mut data = vec![0u8; 9];
        data[0] = 1; // Deposit instruction discriminator (example)
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        Ok(Instruction {
            program_id: self.program_id,
            accounts,
            data,
        })
    }

    fn build_withdraw_instruction(
        &self,
        vault: &Pubkey,
        user_token_account: &Pubkey,
        amount: u64,
    ) -> Result<Instruction> {
        // Placeholder implementation
        let accounts = vec![
            AccountMeta::new(*vault, false),
            AccountMeta::new(*user_token_account, false),
            AccountMeta::new(self.payer.pubkey(), true),
        ];

        let mut data = vec![0u8; 9];
        data[0] = 2; // Withdraw instruction discriminator (example)
        data[1..9].copy_from_slice(&amount.to_le_bytes());

        Ok(Instruction {
            program_id: self.program_id,
            accounts,
            data,
        })
    }

    fn build_harvest_instruction(
        &self,
        vault: &Pubkey,
        user_reward_account: &Pubkey,
    ) -> Result<Instruction> {
        // Placeholder implementation
        let accounts = vec![
            AccountMeta::new(*vault, false),
            AccountMeta::new(*user_reward_account, false),
            AccountMeta::new(self.payer.pubkey(), true),
        ];

        let data = vec![3u8]; // Harvest instruction discriminator (example)

        Ok(Instruction {
            program_id: self.program_id,
            accounts,
            data,
        })
    }

    fn build_compound_instruction(&self, vault: &Pubkey) -> Result<Instruction> {
        // Placeholder implementation
        let accounts = vec![
            AccountMeta::new(*vault, false),
            AccountMeta::new(self.payer.pubkey(), true),
        ];

        let data = vec![4u8]; // Compound instruction discriminator (example)

        Ok(Instruction {
            program_id: self.program_id,
            accounts,
            data,
        })
    }

    /// Get vault information
    pub async fn get_vault_info(&self, vault: &Pubkey) -> Result<()> {
        // Fetch vault account and deserialize
        let _account = self.rpc_client.get_account(vault).await?;
        Ok(())
    }

    /// Calculate expected shares for a deposit amount
    pub fn calculate_shares(&self, deposit_amount: u64, total_deposited: u64, total_shares: u64) -> u64 {
        if total_shares == 0 || total_deposited == 0 {
            return deposit_amount;
        }

        (deposit_amount as u128 * total_shares as u128 / total_deposited as u128) as u64
    }

    /// Calculate expected withdrawal amount for shares
    pub fn calculate_withdrawal(&self, shares: u64, total_deposited: u64, total_shares: u64) -> u64 {
        if total_shares == 0 {
            return 0;
        }

        (shares as u128 * total_deposited as u128 / total_shares as u128) as u64
    }
}

/// Meteora vault information
#[derive(Debug, Clone)]
pub struct MeteoraVaultInfo {
    pub vault_address: Pubkey,
    pub token_mint: Pubkey,
    pub total_deposited: u64,
    pub total_shares: u64,
    pub apy: f64,
    pub fee_bps: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meteora_vault_client_creation() {
        let rpc_client = Arc::new(RpcClient::new("https://api.mainnet-beta.solana.com".to_string()));
        let program_id = Pubkey::new_unique();
        let payer = Arc::new(Keypair::new());
        
        let _client = MeteoraVaultClient::new(rpc_client, program_id, payer);
    }

    #[test]
    fn test_share_calculations() {
        let rpc_client = Arc::new(RpcClient::new("https://api.mainnet-beta.solana.com".to_string()));
        let program_id = Pubkey::new_unique();
        let payer = Arc::new(Keypair::new());
        let client = MeteoraVaultClient::new(rpc_client, program_id, payer);

        // Test initial deposit
        let shares = client.calculate_shares(1000, 0, 0);
        assert_eq!(shares, 1000);

        // Test subsequent deposit
        let shares = client.calculate_shares(1000, 10000, 10000);
        assert_eq!(shares, 1000);

        // Test withdrawal
        let amount = client.calculate_withdrawal(500, 10000, 10000);
        assert_eq!(amount, 500);
    }
}
