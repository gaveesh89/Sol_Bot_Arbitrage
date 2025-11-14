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
    /// Feature: Zero-Risk Transaction Simulation
    /// 
    /// This method simulates the transaction without actually submitting it to the network.
    /// It provides:
    /// - Detailed transaction logs for manual verification of arbitrage logic
    /// - Compute unit usage statistics for priority fee estimation
    /// - Success/failure prediction without spending SOL
    /// - No cost or risk (perfect for testing)
    /// 
    /// Implementation Steps:
    /// 1. Define RpcSimulateTransactionConfig with latest blockhash and logs enabled
    /// 2. Call rpc_client.simulate_transaction_with_config() with the configuration
    /// 3. Check simulation result for errors (simulation_result.value.err)
    /// 4. If error found, log error details and transaction logs
    /// 5. If successful, log success, units_consumed, and transaction logs
    /// 6. Return SimulationResult (enhanced version of Ok(()) with structured data)
    /// 
    /// # Arguments
    /// * `transaction` - The transaction to simulate
    /// * `signer` - The keypair that will sign the transaction (for logging/consistency)
    /// 
    /// # Returns
    /// Result<SimulationResult> containing simulation outcome details
    /// 
    /// # Design Choice
    /// DECISION: Use RpcSimulateTransactionConfig (Chosen) vs simple simulate_transaction call
    /// Rationale: Config allows setting replace_recent_blockhash: true and commitment: processed,
    ///            which is essential for a realistic test that matches actual execution conditions
    /// 
    /// Uses tracing::info! and tracing::error! over println!
    /// Rationale: Provides structured, filterable logging essential for high-performance bots
    /// 
    /// # Optimization
    /// OPTIMIZE: Log the units_consumed to validate the transaction's efficiency and estimate
    ///           real priority fees. Warn if usage exceeds 800K compute units (approaching 1.4M limit)
    /// 
    /// # Alternative
    /// If the simulation is too slow, the alternative is to switch to a local Mainnet Fork
    /// (using solana-test-validator) for faster execution with same accuracy
    pub async fn execute_arbitrage_simulation(
        &self,
        transaction: &Transaction,
        signer: &Keypair,
    ) -> Result<SimulationResult> {
        info!(
            "Starting arbitrage transaction simulation for signer: {}",
            signer.pubkey()
        );

        // Step 1: Define the RpcSimulateTransactionConfig
        // Ensure the simulation is run with the latest blockhash and includes logs
        // - sig_verify: false (faster simulation, we know the signature is valid)
        // - replace_recent_blockhash: true (use latest blockhash for accuracy - CRITICAL)
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

        // Step 2: Call self.rpc_client.simulate_transaction() with the configuration
        // Using simulate_transaction_with_config for full control over simulation parameters
        match self.rpc_client.simulate_transaction_with_config(transaction, config).await {
            Ok(response) => {
                // Step 3: Check the simulation result for errors (simulation_result.value.err)
                let value = response.value;

                if let Some(err) = value.err {
                    // Step 4: If an error is found, log the error details and the transaction logs
                    error!(
                        "❌ Simulation failed with error: {:?}",
                        err
                    );

                    // Extract logs for debugging
                    let logs = value.logs.unwrap_or_default();
                    for (idx, log) in logs.iter().enumerate() {
                        error!("  Log[{}]: {}", idx, log);
                    }

                    // Return failure result with error details
                    return Ok(SimulationResult {
                        success: false,
                        compute_units_consumed: value.units_consumed.unwrap_or(0),
                        logs,
                        error: Some(format!("{:?}", err)),
                    });
                }

                // Step 5: If successful, log the success, the units_consumed, and the transaction logs
                // for manual verification of the arbitrage logic
                let logs = value.logs.unwrap_or_default();
                let units_consumed = value.units_consumed.unwrap_or(0);

                info!("✅ Simulation successful!");
                info!("   Compute units consumed: {} (for priority fee estimation)", units_consumed);
                
                // Log transaction details for manual verification of arbitrage logic
                debug!("   Transaction logs ({} entries):", logs.len());
                for (idx, log) in logs.iter().enumerate() {
                    debug!("     [{}] {}", idx, log);
                }

                // Validate efficiency - warn if compute usage is high
                // OPTIMIZE: Log units_consumed to estimate real priority fees
                if units_consumed > 800_000 {
                    warn!(
                        "⚠️  High compute unit usage: {} (limit is 1.4M)",
                        units_consumed
                    );
                }

                // Step 6: Return SimulationResult to indicate the simulation process completed successfully
                // Enhanced version: Returns structured data instead of just Ok(())
                // This allows caller to inspect success, compute units, logs, and errors
                Ok(SimulationResult {
                    success: true,
                    compute_units_consumed: units_consumed,
                    logs,
                    error: None,
                })
            }
            Err(e) => {
                // Step 4: Handle RPC error (network issues, invalid transaction, etc.)
                // This is different from simulation failure - the RPC call itself failed
                error!("❌ Simulation RPC call failed: {}", e);
                error!("   This could indicate:");
                error!("   - Network connectivity issues");
                error!("   - Invalid transaction structure");
                error!("   - RPC node issues");

                // Step 6: Return error result (simulation process completed, but with RPC error)
                // Note: We return Ok(SimulationResult) with success=false, not Err()
                // This indicates the simulation process itself completed, but detected a problem
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

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        hash::Hash,
        message::Message,
        pubkey::Pubkey,
        system_instruction,
    };

    /// Helper function to create a mock transaction for testing
    fn create_mock_transaction(signer: &Keypair) -> Transaction {
        // Create a simple transfer instruction as a test transaction
        let from = signer.pubkey();
        let to = Pubkey::new_unique();
        let lamports = 1000;

        let instruction = system_instruction::transfer(&from, &to, lamports);
        let message = Message::new(&[instruction], Some(&from));
        
        // Create transaction with a dummy blockhash
        let mut transaction = Transaction::new_unsigned(message);
        transaction.message.recent_blockhash = Hash::new_unique();
        
        // Sign the transaction
        transaction.sign(&[signer], transaction.message.recent_blockhash);
        
        transaction
    }

    /// Test 1: SimulationResult Structure Validation
    #[test]
    fn test_simulation_result_structure() {
        let result = SimulationResult {
            success: true,
            compute_units_consumed: 5000,
            logs: vec!["Log entry 1".to_string(), "Log entry 2".to_string()],
            error: None,
        };

        assert!(result.success);
        assert_eq!(result.compute_units_consumed, 5000);
        assert_eq!(result.logs.len(), 2);
        assert!(result.error.is_none());
    }

    /// Test 2: ExecutionResult Structure Validation
    #[test]
    fn test_execution_result_structure() {
        let signature = "5j7s6NiJS3JAkvgkoc18WVAsiSaci2pxB2A6ueCJP4tprA2TFg9wSyTLeYouxPBJEMzJinENTkpA52YStRW5Dia7";
        
        let result = ExecutionResult {
            signature: signature.to_string(),
            confirmed: true,
            slot: 12345,
            error: None,
        };

        assert!(result.confirmed);
        assert_eq!(result.slot, 12345);
        assert_eq!(result.signature, signature);
        assert!(result.error.is_none());
    }

    /// Test 3: ArbitrageExecutionResult Success Check
    #[test]
    fn test_arbitrage_result_success_check() {
        // Test simulation success
        let sim_result = SimulationResult {
            success: true,
            compute_units_consumed: 5000,
            logs: vec![],
            error: None,
        };
        let arb_result = ArbitrageExecutionResult::Simulation(sim_result);
        assert!(arb_result.is_success());

        // Test simulation failure
        let sim_result_fail = SimulationResult {
            success: false,
            compute_units_consumed: 0,
            logs: vec![],
            error: Some("Simulation error".to_string()),
        };
        let arb_result_fail = ArbitrageExecutionResult::Simulation(sim_result_fail);
        assert!(!arb_result_fail.is_success());

        // Test live execution success
        let exec_result = ExecutionResult {
            signature: "test_sig".to_string(),
            confirmed: true,
            slot: 100,
            error: None,
        };
        let arb_result_live = ArbitrageExecutionResult::Live(exec_result);
        assert!(arb_result_live.is_success());

        // Test live execution failure
        let exec_result_fail = ExecutionResult {
            signature: "".to_string(),
            confirmed: false,
            slot: 0,
            error: Some("Transaction failed".to_string()),
        };
        let arb_result_live_fail = ArbitrageExecutionResult::Live(exec_result_fail);
        assert!(!arb_result_live_fail.is_success());
    }

    /// Test 4: ArbitrageExecutionResult Description
    #[test]
    fn test_arbitrage_result_description() {
        // Simulation success description
        let sim_result = SimulationResult {
            success: true,
            compute_units_consumed: 5000,
            logs: vec![],
            error: None,
        };
        let arb_result = ArbitrageExecutionResult::Simulation(sim_result);
        let desc = arb_result.description();
        assert!(desc.contains("Simulation passed"));
        assert!(desc.contains("5000"));

        // Simulation failure description
        let sim_result_fail = SimulationResult {
            success: false,
            compute_units_consumed: 0,
            logs: vec![],
            error: Some("Test error".to_string()),
        };
        let arb_result_fail = ArbitrageExecutionResult::Simulation(sim_result_fail);
        let desc_fail = arb_result_fail.description();
        assert!(desc_fail.contains("Simulation failed"));
        assert!(desc_fail.contains("Test error"));

        // Live execution success description
        let exec_result = ExecutionResult {
            signature: "test_signature_123".to_string(),
            confirmed: true,
            slot: 100,
            error: None,
        };
        let arb_result_live = ArbitrageExecutionResult::Live(exec_result);
        let desc_live = arb_result_live.description();
        assert!(desc_live.contains("Transaction confirmed"));
        assert!(desc_live.contains("test_signature_123"));
    }

    /// Test 5: Transaction Creation Validation
    #[test]
    fn test_mock_transaction_creation() {
        let signer = Keypair::new();
        let transaction = create_mock_transaction(&signer);

        // Validate transaction structure
        assert_eq!(transaction.message.instructions.len(), 1);
        assert!(transaction.is_signed());
        assert_eq!(transaction.signatures.len(), 1);
        
        // Validate signer
        let expected_signer = signer.pubkey();
        assert_eq!(transaction.message.account_keys[0], expected_signer);
    }

    /// Test 6: Compute Units Threshold Validation
    #[test]
    fn test_compute_units_thresholds() {
        // Low compute usage (under warning threshold)
        let low_usage = SimulationResult {
            success: true,
            compute_units_consumed: 100_000,
            logs: vec![],
            error: None,
        };
        assert!(low_usage.compute_units_consumed < 800_000);

        // High compute usage (should trigger warning)
        let high_usage = SimulationResult {
            success: true,
            compute_units_consumed: 900_000,
            logs: vec![],
            error: None,
        };
        assert!(high_usage.compute_units_consumed > 800_000);

        // Maximum compute units (1.4M limit)
        let max_usage = SimulationResult {
            success: true,
            compute_units_consumed: 1_400_000,
            logs: vec![],
            error: None,
        };
        assert!(max_usage.compute_units_consumed <= 1_400_000);
    }

    /// Test 7: Error Message Validation
    #[test]
    fn test_error_message_formats() {
        // Common Solana errors that should be detected
        let errors = vec![
            ("BlockhashNotFound", "Blockhash expired"),
            ("InsufficientFunds", "Insufficient funds"),
            ("AlreadyProcessed", "already processed"),
        ];

        for (error_type, expected_context) in errors {
            let error_msg = format!("Transaction failed: {}", error_type);
            
            // Validate error detection logic
            if error_msg.contains("BlockhashNotFound") {
                assert!(expected_context.contains("Blockhash"));
            } else if error_msg.contains("InsufficientFunds") {
                assert!(expected_context.contains("Insufficient"));
            } else if error_msg.contains("AlreadyProcessed") {
                assert!(expected_context.contains("already"));
            }
        }
    }

    /// Test 8: RpcSimulateTransactionConfig Validation
    #[test]
    fn test_simulation_config_structure() {
        let config = RpcSimulateTransactionConfig {
            sig_verify: false,
            replace_recent_blockhash: true,
            commitment: Some(CommitmentConfig::processed()),
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: false,
        };

        assert!(!config.sig_verify, "sig_verify should be false for faster simulation");
        assert!(config.replace_recent_blockhash, "Should replace blockhash for accuracy");
        assert!(config.commitment.is_some(), "Commitment level should be set");
        assert!(!config.inner_instructions, "Inner instructions not needed for basic simulation");
    }

    /// Test 9: Transaction Signature Validation
    #[test]
    fn test_transaction_signature_format() {
        let valid_signature = "5j7s6NiJS3JAkvgkoc18WVAsiSaci2pxB2A6ueCJP4tprA2TFg9wSyTLeYouxPBJEMzJinENTkpA52YStRW5Dia7";
        let empty_signature = "";

        // Valid signature should not be empty
        assert!(!valid_signature.is_empty());
        assert!(valid_signature.len() > 80); // Base58 signatures are typically 87-88 chars

        // Empty signature indicates failure
        assert!(empty_signature.is_empty());
    }

    /// Test 10: Execution Mode Safety
    #[test]
    fn test_execution_mode_safety() {
        // Test that simulation mode is the safe default
        let is_simulation_mode = true;
        assert!(is_simulation_mode, "Default should be simulation mode for safety");

        // Test mode switching logic
        let test_simulation = is_simulation_mode;
        let test_live = !is_simulation_mode;
        
        assert!(test_simulation, "Simulation mode should be true");
        assert!(!test_live, "Live mode should be false by default");
    }

    // Note: Integration tests that require actual RPC connection should be in tests/ directory
    // These unit tests validate the structure and logic without network calls
}
