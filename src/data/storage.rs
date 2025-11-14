// Feature: File-Based Data Storage Service
// 
// Tasks (in order):
// 1. Define a struct `TradeStorage` with a `file_path` field (e.g., "trade_history.jsonl").
// 2. Implement a method `save_record(&self, record: &TradeRecord)` that serializes the record to a JSON line and appends it to the file.
// 3. Implement a method `load_all_records(&self) -> Result<Vec<TradeRecord>>` that reads the file line by line, deserializes each JSON line, and returns a vector of all records.

// DECISION: Use JSON Lines (.jsonl) (Chosen) vs a single large JSON file.
// Chosen: JSON Lines allows appending new records without rewriting the entire file, which is faster and safer for continuous logging.

// OPTIMIZE: Use `tokio::fs::File` for asynchronous file I/O to prevent blocking the main bot loop.

// Alternative: Use the `rusqlite` crate to implement a local SQLite database for more robust querying.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, info, warn};

use super::TradeRecord;

/// File-based trade history storage using JSON Lines format
/// 
/// Stores trade records in a .jsonl file where each line is a separate JSON object.
/// This format allows efficient appending without rewriting the entire file.
pub struct TradeStorage {
    /// Path to the JSON Lines file
    file_path: PathBuf,
}

impl TradeStorage {
    /// Create a new TradeStorage instance
    /// 
    /// # Arguments
    /// * `file_path` - Path to the .jsonl file (e.g., "trade_history.jsonl")
    /// 
    /// # Example
    /// ```no_run
    /// use solana_mev_bot::data::storage::TradeStorage;
    /// 
    /// let storage = TradeStorage::new("data/trade_history.jsonl");
    /// ```
    pub fn new<P: AsRef<Path>>(file_path: P) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
        }
    }
    
    /// Ensure the parent directory exists
    async fn ensure_directory(&self) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create storage directory")?;
        }
        Ok(())
    }
    
    /// Save a trade record to the file
    /// 
    /// Serializes the record as JSON and appends it as a new line to the file.
    /// This operation is atomic and thread-safe.
    /// 
    /// # Arguments
    /// * `record` - The trade record to save
    /// 
    /// # Errors
    /// Returns an error if file operations or serialization fail
    /// 
    /// # Example
    /// ```no_run
    /// use solana_mev_bot::data::{TradeRecord, storage::TradeStorage};
    /// 
    /// # async fn example() -> anyhow::Result<()> {
    /// let storage = TradeStorage::new("data/trade_history.jsonl");
    /// let record = TradeRecord::success(
    ///     1699900000000,
    ///     "sig123".to_string(),
    ///     "So11111111111111111111111111111111111111112".to_string(),
    ///     1000000,
    ///     950000,
    ///     150,
    ///     "LIVE".to_string(),
    /// );
    /// 
    /// storage.save_record(&record).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn save_record(&self, record: &TradeRecord) -> Result<()> {
        // Ensure directory exists
        self.ensure_directory().await?;
        
        // Serialize record to JSON
        let json = serde_json::to_string(record)
            .context("Failed to serialize trade record")?;
        
        // Open file in append mode (create if doesn't exist)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await
            .context("Failed to open trade history file")?;
        
        // Write JSON line with newline
        file.write_all(json.as_bytes()).await?;
        file.write_all(b"\n").await?;
        
        // Ensure data is written to disk
        file.flush().await?;
        
        debug!(
            "Saved trade record: success={}, profit={}, file={}",
            record.success,
            record.profit_amount,
            self.file_path.display()
        );
        
        Ok(())
    }
    
    /// Load all trade records from the file
    /// 
    /// Reads the file line by line, deserializing each JSON line into a TradeRecord.
    /// Invalid lines are logged and skipped rather than causing the entire load to fail.
    /// 
    /// # Returns
    /// A vector of all successfully loaded trade records
    /// 
    /// # Errors
    /// Returns an error if the file cannot be opened or read
    /// 
    /// # Example
    /// ```no_run
    /// use solana_mev_bot::data::storage::TradeStorage;
    /// 
    /// # async fn example() -> anyhow::Result<()> {
    /// let storage = TradeStorage::new("data/trade_history.jsonl");
    /// let records = storage.load_all_records().await?;
    /// 
    /// println!("Loaded {} trade records", records.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn load_all_records(&self) -> Result<Vec<TradeRecord>> {
        // Return empty vector if file doesn't exist
        if !self.file_path.exists() {
            info!(
                "Trade history file does not exist yet: {}",
                self.file_path.display()
            );
            return Ok(Vec::new());
        }
        
        // Open file for reading
        let file = File::open(&self.file_path)
            .await
            .context("Failed to open trade history file")?;
        
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        
        let mut records = Vec::new();
        let mut line_number = 0;
        let mut error_count = 0;
        
        // Read and parse each line
        while let Some(line) = lines.next_line().await? {
            line_number += 1;
            
            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }
            
            // Try to deserialize the line
            match serde_json::from_str::<TradeRecord>(&line) {
                Ok(record) => records.push(record),
                Err(e) => {
                    warn!(
                        "Failed to parse line {} in {}: {}",
                        line_number,
                        self.file_path.display(),
                        e
                    );
                    error_count += 1;
                }
            }
        }
        
        info!(
            "Loaded {} trade records from {} ({} parse errors)",
            records.len(),
            self.file_path.display(),
            error_count
        );
        
        Ok(records)
    }
    
    /// Get the total number of records in the file
    /// 
    /// This is a lightweight operation that counts lines without full deserialization.
    pub async fn count_records(&self) -> Result<usize> {
        if !self.file_path.exists() {
            return Ok(0);
        }
        
        let file = File::open(&self.file_path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        
        let mut count = 0;
        while let Some(line) = lines.next_line().await? {
            if !line.trim().is_empty() {
                count += 1;
            }
        }
        
        Ok(count)
    }
    
    /// Get the file path
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
    
    /// Check if the storage file exists
    pub fn exists(&self) -> bool {
        self.file_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_save_and_load_single_record() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_trades.jsonl");
        let storage = TradeStorage::new(&file_path);
        
        let record = TradeRecord::success(
            1699900000000,
            "sig123".to_string(),
            "So11111111111111111111111111111111111111112".to_string(),
            1000000,
            950000,
            150,
            "LIVE".to_string(),
        );
        
        // Save record
        storage.save_record(&record).await.unwrap();
        
        // Load records
        let loaded = storage.load_all_records().await.unwrap();
        
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].timestamp, record.timestamp);
        assert_eq!(loaded[0].signature, record.signature);
        assert_eq!(loaded[0].profit_amount, record.profit_amount);
    }
    
    #[tokio::test]
    async fn test_save_multiple_records() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_trades.jsonl");
        let storage = TradeStorage::new(&file_path);
        
        // Save multiple records
        for i in 0..5 {
            let record = TradeRecord::success(
                1699900000000 + i,
                format!("sig{}", i),
                "So11111111111111111111111111111111111111112".to_string(),
                1000000 + i as u64,
                950000,
                150,
                "LIVE".to_string(),
            );
            storage.save_record(&record).await.unwrap();
        }
        
        // Load all records
        let loaded = storage.load_all_records().await.unwrap();
        
        assert_eq!(loaded.len(), 5);
        assert_eq!(loaded[0].signature, Some("sig0".to_string()));
        assert_eq!(loaded[4].signature, Some("sig4".to_string()));
    }
    
    #[tokio::test]
    async fn test_load_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.jsonl");
        let storage = TradeStorage::new(&file_path);
        
        let loaded = storage.load_all_records().await.unwrap();
        assert_eq!(loaded.len(), 0);
    }
    
    #[tokio::test]
    async fn test_count_records() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_trades.jsonl");
        let storage = TradeStorage::new(&file_path);
        
        // Initially no records
        assert_eq!(storage.count_records().await.unwrap(), 0);
        
        // Add records
        for i in 0..3 {
            let record = TradeRecord::success(
                1699900000000 + i,
                format!("sig{}", i),
                "So11111111111111111111111111111111111111112".to_string(),
                1000000,
                950000,
                150,
                "LIVE".to_string(),
            );
            storage.save_record(&record).await.unwrap();
        }
        
        assert_eq!(storage.count_records().await.unwrap(), 3);
    }
    
    #[tokio::test]
    async fn test_success_and_failure_records() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_trades.jsonl");
        let storage = TradeStorage::new(&file_path);
        
        let success = TradeRecord::success(
            1699900000000,
            "sig_success".to_string(),
            "So11111111111111111111111111111111111111112".to_string(),
            1000000,
            950000,
            150,
            "LIVE".to_string(),
        );
        
        let failure = TradeRecord::failure(
            1699900001000,
            "So11111111111111111111111111111111111111112".to_string(),
            950000,
            50,
            "LIVE".to_string(),
            "Slippage too high".to_string(),
        );
        
        storage.save_record(&success).await.unwrap();
        storage.save_record(&failure).await.unwrap();
        
        let loaded = storage.load_all_records().await.unwrap();
        
        assert_eq!(loaded.len(), 2);
        assert!(loaded[0].success);
        assert!(!loaded[1].success);
        assert_eq!(loaded[1].error_message, Some("Slippage too high".to_string()));
    }
}
