// Feature: Trade Record Data Structure
// 
// Tasks (in order):
// 1. Define a new struct `TradeRecord` to store the outcome of each arbitrage attempt.
// 2. Fields should include: `timestamp`, `signature` (Option<String>), `success` (bool), 
//    `profit_token_mint`, `profit_amount` (u64), `expected_profit_amount` (u64), 
//    `latency_ms` (u64), `execution_mode` (String), `error_message` (Option<String>).
// 3. Derive `Serialize` and `Deserialize` using the `serde` crate for easy persistence.

// DECISION: Use a dedicated struct (Chosen) vs passing raw data.
// Chosen: A dedicated struct ensures data integrity and simplifies serialization/deserialization for storage.

// OPTIMIZE: Include `expected_profit_amount` to track profit variance accurately.

// Alternative: Use a simple JSON string for logging, but a struct is better for structured analysis.

use serde::{Deserialize, Serialize};

/// Trade record capturing the outcome of each arbitrage attempt
/// 
/// This structure stores comprehensive information about executed trades
/// for historical analysis and performance tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// Unix timestamp (milliseconds) when the trade was executed
    pub timestamp: i64,
    
    /// Transaction signature if trade was successfully submitted on-chain
    pub signature: Option<String>,
    
    /// Whether the trade execution was successful
    pub success: bool,
    
    /// Token mint address for the profit token
    pub profit_token_mint: String,
    
    /// Actual profit amount received (in token's smallest unit)
    pub profit_amount: u64,
    
    /// Expected profit amount before execution (in token's smallest unit)
    pub expected_profit_amount: u64,
    
    /// Execution latency in milliseconds (from opportunity detection to tx confirmation)
    pub latency_ms: u64,
    
    /// Execution mode: "LIVE" or "SIMULATION"
    pub execution_mode: String,
    
    /// Error message if the trade failed
    pub error_message: Option<String>,
}

impl TradeRecord {
    /// Create a new successful trade record
    pub fn success(
        timestamp: i64,
        signature: String,
        profit_token_mint: String,
        profit_amount: u64,
        expected_profit_amount: u64,
        latency_ms: u64,
        execution_mode: String,
    ) -> Self {
        Self {
            timestamp,
            signature: Some(signature),
            success: true,
            profit_token_mint,
            profit_amount,
            expected_profit_amount,
            latency_ms,
            execution_mode,
            error_message: None,
        }
    }
    
    /// Create a new failed trade record
    pub fn failure(
        timestamp: i64,
        profit_token_mint: String,
        expected_profit_amount: u64,
        latency_ms: u64,
        execution_mode: String,
        error_message: String,
    ) -> Self {
        Self {
            timestamp,
            signature: None,
            success: false,
            profit_token_mint,
            profit_amount: 0,
            expected_profit_amount,
            latency_ms,
            execution_mode,
            error_message: Some(error_message),
        }
    }
    
    /// Calculate profit variance percentage (actual vs expected)
    pub fn profit_variance_percent(&self) -> f64 {
        if self.expected_profit_amount == 0 {
            return 0.0;
        }
        
        let actual = self.profit_amount as f64;
        let expected = self.expected_profit_amount as f64;
        ((actual - expected) / expected) * 100.0
    }
    
    /// Check if profit met expectations (within acceptable variance threshold)
    pub fn meets_expectations(&self, tolerance_percent: f64) -> bool {
        if !self.success {
            return false;
        }
        
        let variance = self.profit_variance_percent();
        variance.abs() <= tolerance_percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_success_record() {
        let record = TradeRecord::success(
            1699900000000,
            "sig123".to_string(),
            "So11111111111111111111111111111111111111112".to_string(),
            1000000,
            950000,
            150,
            "LIVE".to_string(),
        );
        
        assert!(record.success);
        assert_eq!(record.signature, Some("sig123".to_string()));
        assert_eq!(record.profit_amount, 1000000);
        assert!(record.error_message.is_none());
    }
    
    #[test]
    fn test_failure_record() {
        let record = TradeRecord::failure(
            1699900000000,
            "So11111111111111111111111111111111111111112".to_string(),
            950000,
            50,
            "LIVE".to_string(),
            "Slippage too high".to_string(),
        );
        
        assert!(!record.success);
        assert!(record.signature.is_none());
        assert_eq!(record.profit_amount, 0);
        assert_eq!(record.error_message, Some("Slippage too high".to_string()));
    }
    
    #[test]
    fn test_profit_variance() {
        let record = TradeRecord::success(
            1699900000000,
            "sig123".to_string(),
            "So11111111111111111111111111111111111111112".to_string(),
            1050000,  // 5% more than expected
            1000000,
            150,
            "LIVE".to_string(),
        );
        
        assert_eq!(record.profit_variance_percent(), 5.0);
        assert!(record.meets_expectations(10.0));
        assert!(!record.meets_expectations(3.0));
    }
    
    #[test]
    fn test_serialization() {
        let record = TradeRecord::success(
            1699900000000,
            "sig123".to_string(),
            "So11111111111111111111111111111111111111112".to_string(),
            1000000,
            950000,
            150,
            "LIVE".to_string(),
        );
        
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: TradeRecord = serde_json::from_str(&json).unwrap();
        
        assert_eq!(record.timestamp, deserialized.timestamp);
        assert_eq!(record.signature, deserialized.signature);
        assert_eq!(record.profit_amount, deserialized.profit_amount);
    }
}
