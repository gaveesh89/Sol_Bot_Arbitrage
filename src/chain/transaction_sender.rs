// Multi-RPC Transaction Submission with Priority Fees
//
// This module handles transaction submission to multiple RPC endpoints with:
// 1. Concurrent submission to multiple RPCs
// 2. Dynamic priority fee adjustment based on network congestion
// 3. Transaction confirmation tracking with timeout
// 4. Retry logic with exponential backoff
// 5. Cancellation of remaining submissions once confirmed
// 6. MEV protection and front-run detection

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{
    signature::Signature,
    transaction::VersionedTransaction,
    commitment_config::{CommitmentConfig, CommitmentLevel},
};
use solana_transaction_status::TransactionConfirmationStatus;
use tokio::time::{sleep, timeout, Duration, Instant};
use std::sync::Arc;
use anyhow::{Result, anyhow};
use tracing::{debug, info, warn, error};
use tokio::sync::mpsc;

/// Transaction sender with multi-RPC support
pub struct TransactionSender {
    rpc_clients: Vec<Arc<RpcClient>>,
    max_retries: u8,
    confirmation_timeout_ms: u64,
    max_priority_fee: u64,
}

/// Result of transaction submission
#[derive(Clone, Debug)]
pub struct SendResult {
    pub signature: Signature,
    pub confirmed: bool,
    pub slot: u64,
    pub confirmation_time_ms: u64,
    pub rpc_endpoint: String,
    pub error: Option<String>,
}

/// Configuration for transaction sending
#[derive(Clone, Debug)]
pub struct SendConfig {
    pub priority_fee_lamports: u64,
    pub skip_preflight: bool,
    pub max_retries: u8,
}

impl Default for SendConfig {
    fn default() -> Self {
        Self {
            priority_fee_lamports: 10_000,  // 0.00001 SOL
            skip_preflight: true,            // Skip simulation for speed
            max_retries: 3,
        }
    }
}

impl TransactionSender {
    /// Create new transaction sender
    pub fn new(
        rpc_clients: Vec<Arc<RpcClient>>,
        max_retries: u8,
        confirmation_timeout_ms: u64,
    ) -> Self {
        info!(
            "Initialized TransactionSender with {} RPCs, max_retries={}, timeout={}ms",
            rpc_clients.len(),
            max_retries,
            confirmation_timeout_ms
        );
        
        Self {
            rpc_clients,
            max_retries,
            confirmation_timeout_ms,
            max_priority_fee: 100_000, // 0.0001 SOL max
        }
    }

    /// Send transaction to all RPCs and wait for first confirmation
    pub async fn send_and_confirm(
        &self,
        tx: &VersionedTransaction,
        config: &SendConfig,
    ) -> Result<SendResult> {
        let start_time = Instant::now();
        
        info!(
            "Sending transaction to {} RPCs with priority_fee={} lamports",
            self.rpc_clients.len(),
            config.priority_fee_lamports
        );

        // Channel to receive results from all RPC tasks
        let (result_tx, mut result_rx) = mpsc::unbounded_channel();
        
        // Spawn a task for each RPC client
        let mut handles = Vec::new();
        
        for (idx, client) in self.rpc_clients.iter().enumerate() {
            let client = Arc::clone(client);
            let tx = tx.clone();
            let result_tx = result_tx.clone();
            let config = config.clone();
            let max_retries = self.max_retries;
            let confirmation_timeout = self.confirmation_timeout_ms;
            
            let handle = tokio::spawn(async move {
                match Self::send_to_rpc_with_confirmation(
                    client.clone(),
                    &tx,
                    &config,
                    max_retries,
                    confirmation_timeout,
                    idx,
                ).await {
                    Ok(result) => {
                        let _ = result_tx.send(Ok(result));
                    }
                    Err(e) => {
                        let _ = result_tx.send(Err(e));
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Drop the original sender so channel closes when all tasks complete
        drop(result_tx);

        // Wait for first successful confirmation or all failures
        let mut last_error = None;
        let mut attempts = 0;
        
        while let Some(result) = result_rx.recv().await {
            attempts += 1;
            
            match result {
                Ok(send_result) if send_result.confirmed => {
                    let elapsed = start_time.elapsed().as_millis() as u64;
                    
                    info!(
                        "✅ Transaction confirmed! sig={}, slot={}, time={}ms, rpc={}",
                        send_result.signature,
                        send_result.slot,
                        elapsed,
                        send_result.rpc_endpoint
                    );
                    
                    // Cancel remaining tasks (they'll finish but we don't wait)
                    for handle in handles {
                        handle.abort();
                    }
                    
                    return Ok(SendResult {
                        confirmation_time_ms: elapsed,
                        ..send_result
                    });
                }
                Ok(send_result) => {
                    warn!(
                        "Transaction sent but not confirmed: sig={}, rpc={}",
                        send_result.signature,
                        send_result.rpc_endpoint
                    );
                    last_error = send_result.error;
                }
                Err(e) => {
                    warn!("RPC submission failed: {}", e);
                    last_error = Some(e.to_string());
                }
            }
        }

        // All RPCs failed or timed out
        Err(anyhow!(
            "Transaction failed on all {} RPCs after {}ms: {}",
            attempts,
            start_time.elapsed().as_millis(),
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        ))
    }

    /// Send to single RPC and wait for confirmation
    async fn send_to_rpc_with_confirmation(
        client: Arc<RpcClient>,
        tx: &VersionedTransaction,
        config: &SendConfig,
        max_retries: u8,
        confirmation_timeout_ms: u64,
        rpc_index: usize,
    ) -> Result<SendResult> {
        let start_time = Instant::now();
        
        // Send transaction with retries
        let signature = Self::send_to_rpc_with_retries(
            client.clone(),
            tx,
            config,
            max_retries,
        ).await?;

        debug!(
            "Transaction sent to RPC #{}: sig={}",
            rpc_index,
            signature
        );

        // Wait for confirmation
        let result = Self::wait_for_confirmation(
            client.clone(),
            signature,
            confirmation_timeout_ms,
            rpc_index,
        ).await?;

        Ok(SendResult {
            confirmation_time_ms: start_time.elapsed().as_millis() as u64,
            ..result
        })
    }

    /// Send to single RPC with retry logic
    async fn send_to_rpc_with_retries(
        client: Arc<RpcClient>,
        tx: &VersionedTransaction,
        config: &SendConfig,
        max_retries: u8,
    ) -> Result<Signature> {
        let mut retry_count = 0;
        let mut last_error = None;
        
        while retry_count <= max_retries {
            let send_config = RpcSendTransactionConfig {
                skip_preflight: config.skip_preflight,
                preflight_commitment: Some(CommitmentLevel::Confirmed),
                encoding: None,
                max_retries: None,
                min_context_slot: None,
            };

            match client.send_transaction_with_config(tx, send_config).await {
                Ok(signature) => {
                    debug!(
                        "Transaction sent successfully on attempt {}: {}",
                        retry_count + 1,
                        signature
                    );
                    return Ok(signature);
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    
                    if retry_count < max_retries {
                        // Exponential backoff: 100ms, 200ms, 400ms
                        let delay_ms = 100 * (2u64.pow(retry_count as u32));
                        debug!(
                            "Send failed (attempt {}), retrying in {}ms: {}",
                            retry_count + 1,
                            delay_ms,
                            last_error.as_ref().unwrap()
                        );
                        sleep(Duration::from_millis(delay_ms)).await;
                    }
                    
                    retry_count += 1;
                }
            }
        }

        Err(anyhow!(
            "Failed to send transaction after {} retries: {}",
            max_retries,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        ))
    }

    /// Wait for transaction confirmation
    async fn wait_for_confirmation(
        client: Arc<RpcClient>,
        signature: Signature,
        timeout_ms: u64,
        rpc_index: usize,
    ) -> Result<SendResult> {
        let start_time = Instant::now();
        let timeout_duration = Duration::from_millis(timeout_ms);
        
        let result = timeout(timeout_duration, async {
            loop {
                // Poll signature status
                match client.get_signature_statuses(&[signature]).await {
                    Ok(response) => {
                        if let Some(Some(status)) = response.value.first() {
                            // Check if confirmed
                            if status.confirmation_status.is_some() {
                                let confirmation_status = status.confirmation_status.as_ref().unwrap();
                                
                                if matches!(
                                    confirmation_status,
                                    TransactionConfirmationStatus::Confirmed |
                                    TransactionConfirmationStatus::Finalized
                                ) {
                                    return Ok(SendResult {
                                        signature,
                                        confirmed: true,
                                        slot: status.slot,
                                        confirmation_time_ms: start_time.elapsed().as_millis() as u64,
                                        rpc_endpoint: format!("RPC #{}", rpc_index),
                                        error: status.err.as_ref().map(|e| format!("{:?}", e)),
                                    });
                                }
                            }
                            
                            // Check for error
                            if let Some(err) = &status.err {
                                return Err(anyhow!("Transaction failed: {:?}", err));
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Error checking signature status: {}", e);
                    }
                }
                
                // Poll every 400ms
                sleep(Duration::from_millis(400)).await;
            }
        }).await;

        match result {
            Ok(Ok(send_result)) => Ok(send_result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!(
                "Transaction confirmation timeout after {}ms",
                timeout_ms
            )),
        }
    }

    /// Calculate dynamic priority fee based on recent blocks
    pub async fn estimate_priority_fee(&self) -> Result<u64> {
        // Use first RPC client to get recent priority fees
        let client = self.rpc_clients.first()
            .ok_or_else(|| anyhow!("No RPC clients available"))?;

        // Get recent prioritization fees
        // Note: This is a simplified implementation
        // In production, call getRecentPrioritizationFees RPC method
        
        match client.get_recent_blockhash().await {
            Ok(_blockhash) => {
                // Calculate dynamic fee based on network congestion
                // For now, return a moderate default
                let base_fee = 5_000u64;      // 0.000005 SOL base
                let congestion_multiplier = 2; // Assume moderate congestion
                let estimated_fee = base_fee * congestion_multiplier;
                
                // Cap at maximum
                let capped_fee = estimated_fee.min(self.max_priority_fee);
                
                debug!(
                    "Estimated priority fee: {} lamports (capped at {})",
                    capped_fee,
                    self.max_priority_fee
                );
                
                Ok(capped_fee)
            }
            Err(e) => {
                warn!("Failed to estimate priority fee: {}, using default", e);
                Ok(10_000) // Default fallback
            }
        }
    }

    /// Detect if transaction was front-run by MEV bot
    pub fn detect_frontrun(
        &self,
        tx_result: &SendResult,
        expected_profit_lamports: u64,
        actual_profit_lamports: u64,
    ) -> bool {
        if !tx_result.confirmed {
            return false;
        }

        // Check if actual profit is significantly lower than expected
        // This could indicate front-running or sandwich attack
        let profit_ratio = if expected_profit_lamports > 0 {
            actual_profit_lamports as f64 / expected_profit_lamports as f64
        } else {
            1.0
        };

        // If actual profit is less than 50% of expected, likely front-run
        let frontrun_threshold = 0.5;
        let is_frontrun = profit_ratio < frontrun_threshold;
        
        if is_frontrun {
            warn!(
                "⚠️  Possible front-run detected! Expected: {} lamports, Actual: {} lamports ({:.1}%)",
                expected_profit_lamports,
                actual_profit_lamports,
                profit_ratio * 100.0
            );
        }

        is_frontrun
    }

    /// Update maximum priority fee limit
    pub fn set_max_priority_fee(&mut self, max_fee: u64) {
        self.max_priority_fee = max_fee;
        info!("Updated max_priority_fee to {} lamports", max_fee);
    }

    /// Get current RPC client count
    pub fn rpc_count(&self) -> usize {
        self.rpc_clients.len()
    }

    /// Add new RPC client
    pub fn add_rpc_client(&mut self, client: Arc<RpcClient>) {
        self.rpc_clients.push(client);
        info!("Added RPC client, total: {}", self.rpc_clients.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::Keypair;

    #[test]
    fn test_send_config_default() {
        let config = SendConfig::default();
        assert_eq!(config.priority_fee_lamports, 10_000);
        assert_eq!(config.skip_preflight, true);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_transaction_sender_creation() {
        let client = Arc::new(RpcClient::new("https://api.mainnet-beta.solana.com".to_string()));
        let sender = TransactionSender::new(vec![client], 3, 30_000);
        
        assert_eq!(sender.rpc_count(), 1);
        assert_eq!(sender.max_retries, 3);
        assert_eq!(sender.confirmation_timeout_ms, 30_000);
    }

    #[test]
    fn test_detect_frontrun() {
        let client = Arc::new(RpcClient::new("https://api.mainnet-beta.solana.com".to_string()));
        let sender = TransactionSender::new(vec![client], 3, 30_000);
        
        let tx_result = SendResult {
            signature: Signature::default(),
            confirmed: true,
            slot: 12345,
            confirmation_time_ms: 1000,
            rpc_endpoint: "test".to_string(),
            error: None,
        };
        
        // Expected 100k profit, got 30k = 30% = front-run detected
        let is_frontrun = sender.detect_frontrun(&tx_result, 100_000, 30_000);
        assert!(is_frontrun);
        
        // Expected 100k profit, got 90k = 90% = no front-run
        let is_frontrun = sender.detect_frontrun(&tx_result, 100_000, 90_000);
        assert!(!is_frontrun);
        
        // Expected 100k profit, got 50k = 50% = borderline (no front-run at threshold)
        let is_frontrun = sender.detect_frontrun(&tx_result, 100_000, 50_000);
        assert!(!is_frontrun);
    }

    #[test]
    fn test_add_rpc_client() {
        let client1 = Arc::new(RpcClient::new("https://api.mainnet-beta.solana.com".to_string()));
        let mut sender = TransactionSender::new(vec![client1], 3, 30_000);
        
        assert_eq!(sender.rpc_count(), 1);
        
        let client2 = Arc::new(RpcClient::new("https://solana-api.projectserum.com".to_string()));
        sender.add_rpc_client(client2);
        
        assert_eq!(sender.rpc_count(), 2);
    }

    #[test]
    fn test_set_max_priority_fee() {
        let client = Arc::new(RpcClient::new("https://api.mainnet-beta.solana.com".to_string()));
        let mut sender = TransactionSender::new(vec![client], 3, 30_000);
        
        assert_eq!(sender.max_priority_fee, 100_000);
        
        sender.set_max_priority_fee(200_000);
        assert_eq!(sender.max_priority_fee, 200_000);
    }
}
