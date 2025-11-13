use anyhow::{Context, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    // signer::Signer,
    // system_instruction,
    transaction::{Transaction, VersionedTransaction},
};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Transaction builder with optimization features
pub struct TransactionBuilder {
    instructions: Vec<Instruction>,
    payer: Pubkey,
    compute_unit_limit: Option<u32>,
    compute_unit_price: Option<u64>,
    priority_fee: Option<u64>,
}

impl TransactionBuilder {
    pub fn new(payer: Pubkey) -> Self {
        Self {
            instructions: Vec::new(),
            payer,
            compute_unit_limit: None,
            compute_unit_price: None,
            priority_fee: None,
        }
    }

    /// Add an instruction to the transaction
    pub fn add_instruction(&mut self, instruction: Instruction) -> &mut Self {
        self.instructions.push(instruction);
        self
    }

    /// Add multiple instructions to the transaction
    pub fn add_instructions(&mut self, instructions: Vec<Instruction>) -> &mut Self {
        self.instructions.extend(instructions);
        self
    }

    /// Set compute unit limit
    pub fn set_compute_unit_limit(&mut self, limit: u32) -> &mut Self {
        self.compute_unit_limit = Some(limit);
        self
    }

    /// Set compute unit price (micro-lamports per compute unit)
    pub fn set_compute_unit_price(&mut self, price: u64) -> &mut Self {
        self.compute_unit_price = Some(price);
        self
    }

    /// Set priority fee in lamports
    pub fn set_priority_fee(&mut self, fee: u64) -> &mut Self {
        self.priority_fee = Some(fee);
        self
    }

    /// Build a legacy transaction
    pub async fn build(
        &self,
        rpc_client: &RpcClient,
        signer: &Keypair,
    ) -> Result<Transaction> {
        let mut instructions = Vec::new();

        // Add compute budget instructions if specified
        if let Some(limit) = self.compute_unit_limit {
            instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(limit));
        }

        if let Some(price) = self.compute_unit_price {
            instructions.push(ComputeBudgetInstruction::set_compute_unit_price(price));
        }

        // Add user instructions
        instructions.extend(self.instructions.clone());

        let recent_blockhash = rpc_client
            .get_latest_blockhash()
            .await
            .context("Failed to get recent blockhash")?;

        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.payer),
            &[signer],
            recent_blockhash,
        );

        debug!(
            "Built transaction with {} instructions",
            instructions.len()
        );

        Ok(transaction)
    }

    /// Build a versioned transaction (optimized with address lookup tables)
    pub async fn build_with_alt(
        self,
        rpc_client: Arc<RpcClient>,
        _lookup_tables: Vec<Pubkey>,
        signer: &Keypair,
    ) -> Result<VersionedTransaction> {
        let mut instructions = Vec::new();

        // Add compute budget instructions if specified
        if let Some(limit) = self.compute_unit_limit {
            instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(limit));
        }

        if let Some(price) = self.compute_unit_price {
            instructions.push(ComputeBudgetInstruction::set_compute_unit_price(price));
        }

        // Add user instructions
        instructions.extend(self.instructions.clone());

        // Note: Full implementation would require fetching and using address lookup tables
        // This is a simplified version
        warn!("Versioned transaction building with ALT is not fully implemented");

        let _recent_blockhash = rpc_client
            .get_latest_blockhash()
            .await
            .context("Failed to get recent blockhash")?;

        let message = Message::new(&instructions, Some(&self.payer));
        let versioned_message = solana_sdk::message::VersionedMessage::Legacy(message);

        let versioned_tx = VersionedTransaction::try_new(versioned_message, &[signer])
            .context("Failed to create versioned transaction")?;

        debug!(
            "Built versioned transaction with {} instructions",
            instructions.len()
        );

        Ok(versioned_tx)
    }

    /// Simulate transaction before sending
    pub async fn simulate(&self, rpc_client: &RpcClient, signer: &Keypair) -> Result<()> {
        let transaction = self.build(rpc_client, signer).await?;

        let simulation = rpc_client
            .simulate_transaction(&transaction)
            .await
            .context("Failed to simulate transaction")?;

        if let Some(err) = simulation.value.err {
            return Err(anyhow::anyhow!("Transaction simulation failed: {:?}", err));
        }

        info!("Transaction simulation successful");
        Ok(())
    }

    /// Clear all instructions
    pub fn clear(&mut self) {
        self.instructions.clear();
    }
}

/// Multi-RPC transaction sender for increased inclusion probability
pub struct MultiRpcSender {
    rpc_clients: Vec<Arc<RpcClient>>,
}

impl MultiRpcSender {
    pub fn new(rpc_urls: Vec<String>) -> Self {
        let rpc_clients = rpc_urls
            .into_iter()
            .map(|url| Arc::new(RpcClient::new(url)))
            .collect();

        Self { rpc_clients }
    }

    /// Send transaction to multiple RPCs in parallel
    pub async fn send_transaction_multiple(
        &self,
        transaction: &Transaction,
    ) -> Result<Vec<Result<Signature>>> {
        let mut tasks = Vec::new();

        for rpc_client in &self.rpc_clients {
            let client = Arc::clone(rpc_client);
            let tx = transaction.clone();

            let task = tokio::spawn(async move {
                client
                    .send_and_confirm_transaction_with_spinner_and_config(
                        &tx,
                        CommitmentConfig::confirmed(),
                        Default::default(),
                    )
                    .await
                    .context("Failed to send transaction")
            });

            tasks.push(task);
        }

        let results = futures::future::join_all(tasks).await;

        let signatures: Vec<Result<Signature>> = results
            .into_iter()
            .map(|r| match r {
                Ok(Ok(sig)) => Ok(sig),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(anyhow::anyhow!("Task failed: {}", e)),
            })
            .collect();

        info!(
            "Sent transaction to {} RPCs, {} succeeded",
            self.rpc_clients.len(),
            signatures.iter().filter(|r| r.is_ok()).count()
        );

        Ok(signatures)
    }

    /// Send and get first successful signature
    pub async fn send_and_get_first_success(
        &self,
        transaction: &Transaction,
    ) -> Result<Signature> {
        let results = self.send_transaction_multiple(transaction).await?;

        for result in results {
            if let Ok(sig) = result {
                info!("Transaction confirmed: {}", sig);
                return Ok(sig);
            }
        }

        Err(anyhow::anyhow!("All RPC submissions failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_builder_creation() {
        let payer = Pubkey::new_unique();
        let mut builder = TransactionBuilder::new(payer);
        
        builder.set_compute_unit_limit(200000);
        builder.set_compute_unit_price(1000);
        
        assert_eq!(builder.compute_unit_limit, Some(200000));
        assert_eq!(builder.compute_unit_price, Some(1000));
    }

    #[test]
    fn test_multi_rpc_sender_creation() {
        let urls = vec![
            "https://api.mainnet-beta.solana.com".to_string(),
            "https://solana-api.projectserum.com".to_string(),
        ];
        
        let sender = MultiRpcSender::new(urls);
        assert_eq!(sender.rpc_clients.len(), 2);
    }
}
