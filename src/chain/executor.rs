use anyhow::Result;
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::RpcSimulateTransactionConfig,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Transaction executor for arbitrage operations
/// 
/// This module handles the execution of arbitrage transactions with two modes:
/// 1. Simulation mode - Zero-risk testing using RPC simulate_transaction
/// 2. Live mode - Real execution on-chain (to be implemented)
/// 
/// Design Choice: Using simulate_transaction over send_and_confirm_transaction
/// Rationale: Simulation provides detailed logs and compute unit usage without
/// spending real money, making it ideal for testing and validation.
pub struct TransactionExecutor {
    rpc_client: Arc<RpcClient>,
}

impl TransactionExecutor {
    /// Create a new transaction executor
    /// 
    /// # Arguments
    /// * `rpc_client` - Shared RPC client for Solana network communication
    /// 
    /// # Returns
    /// A new TransactionExecutor instance
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        info!("Initializing TransactionExecutor");
        Self { rpc_client }
    }

    /// Execute arbitrage transaction in simulation mode (zero-risk testing)
    /// 
    /// This method simulates the transaction without actually submitting it to the network.
    /// It provides:
    /// - Detailed transaction logs
    /// - Compute unit usage statistics
    /// - Success/failure prediction
    /// - No cost or risk
    /// 
    /// # Arguments
    /// * `transaction` - The transaction to simulate
    /// * `signer` - The keypair that will sign the transaction
    /// 
    /// # Returns
    /// Result containing simulation outcome details
    /// 
    /// # Design Choice
    /// Uses tracing::info! and tracing::error! over println!
    /// Rationale: Provides structured, filterable logging essential for high-performance bots
    /// 
    /// # Optimization
    /// Logs exact units_consumed to validate transaction efficiency
    /// 
    /// # Alternative
    /// If simulate_transaction is unavailable or unreliable, fallback to solana-test-validator
    pub async fn execute_arbitrage_simulation(
        &self,
        transaction: &Transaction,
        signer: &Keypair,
    ) -> Result<SimulationResult> {
        info!(
            "Starting arbitrage transaction simulation for signer: {}",
            signer.pubkey()
        );

        // Step 2: Create RpcSimulateTransactionConfig
        // - sig_verify: false (faster simulation, we know the signature is valid)
        // - replace_recent_blockhash: true (use latest blockhash for accuracy)
        // - commitment: Processed (fastest feedback, suitable for simulation)
        // - encoding: Base64 (default, efficient)
        // - accounts: None (we don't need account state in response)
        let config = RpcSimulateTransactionConfig {
            sig_verify: false,
            replace_recent_blockhash: true,
            commitment: Some(CommitmentConfig::processed()),
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: false,
        };

        debug!(
            "Simulation config: sig_verify=false, replace_blockhash=true, commitment=processed"
        );

        // Step 3: Call simulate_transaction with config
        match self.rpc_client.simulate_transaction_with_config(transaction, config).await {
            Ok(response) => {
                // Step 4: Handle successful response
                let value = response.value;

                if let Some(err) = value.err {
                    // Simulation failed - log error details
                    error!(
                        "❌ Simulation failed with error: {:?}",
                        err
                    );

                    // Extract logs for debugging
                    let logs = value.logs.unwrap_or_default();
                    for (idx, log) in logs.iter().enumerate() {
                        error!("  Log[{}]: {}", idx, log);
                    }

                    // Step 5: Return failure result
                    return Ok(SimulationResult {
                        success: false,
                        compute_units_consumed: value.units_consumed.unwrap_or(0),
                        logs,
                        error: Some(format!("{:?}", err)),
                    });
                }

                // Simulation succeeded - extract logs and compute units
                let logs = value.logs.unwrap_or_default();
                let units_consumed = value.units_consumed.unwrap_or(0);

                info!("✅ Simulation successful!");
                info!("   Compute units consumed: {}", units_consumed);
                
                // Log transaction details for analysis
                debug!("   Transaction logs ({} entries):", logs.len());
                for (idx, log) in logs.iter().enumerate() {
                    debug!("     [{}] {}", idx, log);
                }

                // Validate efficiency - warn if compute usage is high
                if units_consumed > 800_000 {
                    warn!(
                        "⚠️  High compute unit usage: {} (limit is 1.4M)",
                        units_consumed
                    );
                }

                // Step 5: Return success result
                Ok(SimulationResult {
                    success: true,
                    compute_units_consumed: units_consumed,
                    logs,
                    error: None,
                })
            }
            Err(e) => {
                // Step 4: Handle RPC error (network issues, invalid transaction, etc.)
                error!("❌ Simulation RPC call failed: {}", e);
                error!("   This could indicate:");
                error!("   - Network connectivity issues");
                error!("   - Invalid transaction structure");
                error!("   - RPC node issues");

                // Step 5: Return error result
                Ok(SimulationResult {
                    success: false,
                    compute_units_consumed: 0,
                    logs: vec![],
                    error: Some(format!("RPC error: {}", e)),
                })
            }
        }
    }

    /// Execute arbitrage transaction in live mode (real execution)
    /// 
    /// This method submits the transaction to the Solana network for actual execution.
    /// Uses send_and_confirm_transaction for reliable confirmation.
    /// 
    /// # Arguments
    /// * `transaction` - The transaction to execute
    /// * `signer` - The keypair that signed the transaction
    /// 
    /// # Returns
    /// Result containing execution outcome and transaction signature
    /// 
    /// # Design Choice
    /// Uses send_and_confirm_transaction (Chosen) vs send_transaction + manual confirmation
    /// Rationale: Simpler for initial testing, ensures transaction is finalized before proceeding
    /// 
    /// # Optimization
    /// For production, refactor to use send_transaction with separate confirmation thread
    /// for lower latency and better throughput
    /// 
    /// # Alternative
    /// If Jito is integrated, use Jito's bundle submission API instead of standard RPC
    pub async fn execute_arbitrage_live(
        &self,
        transaction: &Transaction,
        signer: &Keypair,
    ) -> Result<ExecutionResult> {
        info!("🚀 Starting LIVE transaction execution");
        warn!("⚠️  LIVE MODE: This will submit real transactions to the network");
        
        // Log transaction details for audit trail
        info!("   Signer: {}", signer.pubkey());
        info!("   Recent blockhash: {}", transaction.message.recent_blockhash);
        info!("   Instructions: {}", transaction.message.instructions.len());
        
        // Submit transaction and wait for confirmation
        match self.rpc_client.send_and_confirm_transaction(transaction).await {
            Ok(signature) => {
                info!("✅ Transaction confirmed!");
                info!("   Signature: {}", signature);
                
                // Fetch transaction details for complete audit trail
                // Note: Using JSON encoding for transaction details
                match self.rpc_client
                    .get_transaction_with_config(
                        &signature,
                        solana_client::rpc_config::RpcTransactionConfig {
                            encoding: Some(solana_transaction_status::UiTransactionEncoding::Json),
                            commitment: Some(CommitmentConfig::confirmed()),
                            max_supported_transaction_version: Some(0),
                        },
                    )
                    .await
                {
                    Ok(confirmed_tx) => {
                        let slot = confirmed_tx.slot;
                        
                        info!("   Slot: {}", slot);
                        
                        // Log compute units if available
                        if let Some(meta) = confirmed_tx.transaction.meta {
                            match meta.compute_units_consumed {
                                solana_transaction_status::option_serializer::OptionSerializer::Some(compute_units) => {
                                    info!("   Compute units consumed: {}", compute_units);
                                }
                                _ => {
                                    debug!("   Compute units not available");
                                }
                            }
                            
                            // Check for errors in transaction execution
                            if let Some(err) = meta.err {
                                error!("❌ Transaction executed but contained error: {:?}", err);
                                return Ok(ExecutionResult {
                                    signature: signature.to_string(),
                                    confirmed: true,
                                    slot,
                                    error: Some(format!("{:?}", err)),
                                });
                            }
                        }
                        
                        info!("💰 Transaction executed successfully on-chain");
                        
                        Ok(ExecutionResult {
                            signature: signature.to_string(),
                            confirmed: true,
                            slot,
                            error: None,
                        })
                    }
                    Err(e) => {
                        // Transaction confirmed but couldn't fetch details
                        warn!("Transaction confirmed but failed to fetch details: {}", e);
                        
                        Ok(ExecutionResult {
                            signature: signature.to_string(),
                            confirmed: true,
                            slot: 0, // Unknown
                            error: None,
                        })
                    }
                }
            }
            Err(e) => {
                error!("❌ Transaction failed: {}", e);
                error!("   Error type: {:?}", e);
                
                // Provide detailed error context
                let error_msg = format!("{}", e);
                if error_msg.contains("BlockhashNotFound") {
                    error!("   Cause: Blockhash expired - transaction took too long to submit");
                } else if error_msg.contains("InsufficientFunds") {
                    error!("   Cause: Insufficient funds for transaction");
                } else if error_msg.contains("AlreadyProcessed") {
                    error!("   Cause: Transaction already processed (possible duplicate)");
                }
                
                Ok(ExecutionResult {
                    signature: String::new(),
                    confirmed: false,
                    slot: 0,
                    error: Some(error_msg),
                })
            }
        }
    }

    /// Execute arbitrage transaction with mode selection
    /// 
    /// This is the main public method that routes to either simulation or live execution
    /// based on the is_simulation_mode flag.
    /// 
    /// # Arguments
    /// * `transaction` - The transaction to execute
    /// * `signer` - The keypair that signed the transaction
    /// * `is_simulation_mode` - If true, simulate only. If false, execute live.
    /// 
    /// # Returns
    /// Result containing either SimulationResult or ExecutionResult wrapped in an enum
    /// 
    /// # Safety
    /// Always check is_simulation_mode flag before calling this method.
    /// Default should be true to prevent accidental live execution.
    pub async fn execute_arbitrage(
        &self,
        transaction: &Transaction,
        signer: &Keypair,
        is_simulation_mode: bool,
    ) -> Result<ArbitrageExecutionResult> {
        if is_simulation_mode {
            info!("🧪 Execution mode: SIMULATION (zero-risk)");
            let result = self.execute_arbitrage_simulation(transaction, signer).await?;
            Ok(ArbitrageExecutionResult::Simulation(result))
        } else {
            warn!("⚠️  Execution mode: LIVE (real funds at risk)");
            warn!("⚠️  This will submit a real transaction to the Solana network");
            let result = self.execute_arbitrage_live(transaction, signer).await?;
            Ok(ArbitrageExecutionResult::Live(result))
        }
    }
}

/// Result of transaction simulation
#[derive(Debug, Clone)]
pub struct SimulationResult {
    /// Whether the simulation succeeded
    pub success: bool,
    
    /// Number of compute units consumed
    pub compute_units_consumed: u64,
    
    /// Transaction logs from simulation
    pub logs: Vec<String>,
    
    /// Error message if simulation failed
    pub error: Option<String>,
}

/// Result of live transaction execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Transaction signature
    pub signature: String,
    
    /// Whether the transaction was confirmed
    pub confirmed: bool,
    
    /// Slot in which transaction was processed
    pub slot: u64,
    
    /// Error message if execution failed
    pub error: Option<String>,
}

/// Combined result type for arbitrage execution
/// Returned by execute_arbitrage() to indicate whether simulation or live execution was performed
#[derive(Debug, Clone)]
pub enum ArbitrageExecutionResult {
    /// Simulation was performed (zero-risk testing)
    Simulation(SimulationResult),
    
    /// Live execution was performed (real on-chain transaction)
    Live(ExecutionResult),
}

impl ArbitrageExecutionResult {
    /// Check if the execution was successful
    pub fn is_success(&self) -> bool {
        match self {
            ArbitrageExecutionResult::Simulation(result) => result.success,
            ArbitrageExecutionResult::Live(result) => result.confirmed && result.error.is_none(),
        }
    }
    
    /// Get a human-readable description of the result
    pub fn description(&self) -> String {
        match self {
            ArbitrageExecutionResult::Simulation(result) => {
                if result.success {
                    format!("Simulation passed ({}CU)", result.compute_units_consumed)
                } else {
                    format!("Simulation failed: {:?}", result.error)
                }
            }
            ArbitrageExecutionResult::Live(result) => {
                if result.confirmed && result.error.is_none() {
                    format!("Transaction confirmed: {}", result.signature)
                } else if result.error.is_some() {
                    format!("Transaction failed: {:?}", result.error)
                } else {
                    format!("Transaction pending: {}", result.signature)
                }
            }
        }
    }
}

// Alternative Implementation Notes:
// 
// 1. If RPC simulation proves too slow or unreliable, consider using:
//    - Local Mainnet Fork (using solana-test-validator --clone)
//      Pros: Faster, more reliable, full state access
//      Cons: Setup complexity, resource intensive, state may drift
//    - Anchor's BanksClient for testing
//      Pros: Fast, integrated testing
//      Cons: Requires test environment setup
//
// 2. For production live execution optimization:
//    - Use send_transaction with separate confirmation thread for lower latency
//    - Implement retry logic with exponential backoff
//    - Add priority fee adjustment based on network congestion
//    - Consider Jito bundle submission for MEV protection
//
// 3. For Jito integration:
//    - Replace send_and_confirm_transaction with Jito's bundle API
//    - Bundle multiple transactions for atomic execution
//    - Pay tips to validators for priority inclusion
//    - Protects against frontrunning and sandwich attacks

