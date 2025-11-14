# Phase 3 Analytics - Quick Start Guide

## What You Have Now

Your MEV bot now automatically tracks and analyzes all trade executions. Every time a trade is attempted (success or failure), it's recorded to `./data/trade_history.jsonl` with detailed metrics.

## Automatic Features (Already Working)

✅ **Automatic Recording**: Every trade execution is captured  
✅ **Persistent Storage**: Data survives bot restarts  
✅ **Non-Blocking I/O**: Storage never slows down trading  
✅ **Error Tolerance**: Corrupted records don't break the system  

## File Locations

```
./data/trade_history.jsonl    # All trade records (auto-created)
./src/data/                   # Data structures and storage
./src/reporting/              # Analytics and report generation
```

## Quick Commands

### 1. View Recent Trades

```bash
# Last 10 trades
tail -10 ./data/trade_history.jsonl | jq '.'

# Last 10 successful trades
grep '"success":true' ./data/trade_history.jsonl | tail -10 | jq '.'

# Last 10 failures
grep '"success":false' ./data/trade_history.jsonl | tail -10 | jq '.'
```

### 2. Quick Statistics

```bash
# Total trades
wc -l ./data/trade_history.jsonl

# Success count
grep -c '"success":true' ./data/trade_history.jsonl

# Failure count
grep -c '"success":false' ./data/trade_history.jsonl

# Success rate
echo "scale=2; $(grep -c '"success":true' ./data/trade_history.jsonl) * 100 / $(wc -l < ./data/trade_history.jsonl)" | bc
```

### 3. Search by Error

```bash
# Find all "Slippage exceeded" errors
grep '"error_message":".*Slippage' ./data/trade_history.jsonl

# Count by error type
grep '"success":false' ./data/trade_history.jsonl | \
  jq -r '.error_message' | sort | uniq -c | sort -rn
```

## Generate Performance Report (In Code)

Add this function to your `main.rs`:

```rust
use crate::{data::TradeStorage, reporting};

async fn print_performance_report(storage: &TradeStorage) -> anyhow::Result<()> {
    let report = reporting::generate_report(storage).await?;
    println!("\n{}\n", report);
    
    // Optional: save to file
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("./reports/report_{}.md", timestamp);
    tokio::fs::create_dir_all("./reports").await?;
    tokio::fs::write(&filename, &report).await?;
    tracing::info!("Report saved: {}", filename);
    
    Ok(())
}
```

### Call it after trading session:

```rust
// In your main loop, after bot runs
print_performance_report(&storage).await?;
```

## Periodic Reports (Recommended)

Add this to your `main.rs` to generate hourly reports:

```rust
use tokio::time::{interval, Duration};

// After creating storage...
let storage_for_reports = storage.clone();
tokio::spawn(async move {
    let mut ticker = interval(Duration::from_secs(3600)); // 1 hour
    loop {
        ticker.tick().await;
        
        match reporting::generate_report(&storage_for_reports).await {
            Ok(report) => {
                tracing::info!("\n=== HOURLY PERFORMANCE REPORT ===\n{}", report);
                
                // Save to file
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                let filename = format!("./reports/hourly_{}.md", timestamp);
                tokio::fs::create_dir_all("./reports").await.ok();
                tokio::fs::write(&filename, report).await.ok();
            }
            Err(e) => tracing::error!("Report generation failed: {}", e),
        }
    }
});
```

## CLI Commands (Enhancement)

You can add these to a CLI interface:

```rust
#[derive(Parser)]
enum Command {
    /// Run the MEV bot
    Run,
    
    /// Generate performance report
    Report {
        #[arg(short, long, default_value = "./data/trade_history.jsonl")]
        input: String,
        
        #[arg(short, long)]
        output: Option<String>,
    },
    
    /// Show trade statistics
    Stats,
    
    /// Export data to CSV
    Export {
        #[arg(short, long, default_value = "./export.csv")]
        output: String,
    },
}

async fn handle_command(cmd: Command) -> anyhow::Result<()> {
    match cmd {
        Command::Run => {
            // Your existing bot logic
            run_bot().await?;
        }
        
        Command::Report { input, output } => {
            let storage = TradeStorage::new(input);
            let report = reporting::generate_report(&storage).await?;
            
            if let Some(path) = output {
                tokio::fs::write(&path, &report).await?;
                println!("Report saved to: {}", path);
            } else {
                println!("{}", report);
            }
        }
        
        Command::Stats => {
            let storage = TradeStorage::new("./data/trade_history.jsonl".to_string());
            let records = storage.load_all_records().await?;
            
            let total = records.len();
            let success = records.iter().filter(|r| r.success).count();
            let failed = total - success;
            let rate = if total > 0 {
                (success as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            
            println!("Trade Statistics:");
            println!("  Total:   {}", total);
            println!("  Success: {} ({:.2}%)", success, rate);
            println!("  Failed:  {}", failed);
        }
        
        Command::Export { output } => {
            let storage = TradeStorage::new("./data/trade_history.jsonl".to_string());
            export_to_csv(&storage, &output).await?;
            println!("Exported to: {}", output);
        }
    }
    Ok(())
}

async fn export_to_csv(storage: &TradeStorage, path: &str) -> anyhow::Result<()> {
    let records = storage.load_all_records().await?;
    let mut csv = String::from("timestamp,success,signature,profit_token,profit_amount,expected_amount,latency_ms,mode,error\n");
    
    for r in records {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            r.timestamp,
            r.success,
            r.signature.as_deref().unwrap_or(""),
            r.profit_token_mint,
            r.profit_amount,
            r.expected_profit_amount,
            r.latency_ms,
            r.execution_mode,
            r.error_message.as_deref().unwrap_or("")
        ));
    }
    
    tokio::fs::write(path, csv).await?;
    Ok(())
}
```

## Example Usage

```bash
# Run bot (records trades automatically)
cargo run --release -- run

# Generate report after session
cargo run --release -- report

# Save report to file
cargo run --release -- report --output ./my_report.md

# Quick stats
cargo run --release -- stats

# Export to CSV for Excel/Google Sheets
cargo run --release -- export --output ./trades.csv
```

## Understanding the Report

### Executive Summary
- Shows total trades and success rate
- Average latency indicates execution speed

### Profit Analysis
- **Total Profit**: Sum of all profits
- **Average Profit**: Typical profit per trade
- **Profit Std Dev**: Measures consistency (lower is better)
- **Coefficient of Variation**: Risk metric (lower is better)

### Risk Assessment
- **Low Risk (CV < 50%)**: Strategy is stable
- **Medium Risk (50-100%)**: Some variability, monitor
- **High Risk (CV > 100%)**: High volatility, consider adjustments

### Failure Analysis
- Top 3 failure reasons with counts
- Helps identify systemic issues
- Use to prioritize bug fixes

## Monitoring Best Practices

### Daily
```bash
# Check today's success rate
grep "$(date +%Y-%m-%d)" ./data/trade_history.jsonl | \
  jq -s '[.[] | .success] | add / length * 100'
```

### Weekly
```bash
# Generate weekly report
cargo run --release -- report --output "./reports/week_$(date +%U).md"
```

### Monthly
```bash
# Backup data
cp ./data/trade_history.jsonl "./backups/$(date +%Y%m).jsonl"

# Analyze trends
cargo run --release -- report --output "./reports/month_$(date +%Y%m).md"
```

## Maintenance

### Log Rotation (When file > 100MB)

```bash
# Backup old data
mkdir -p ./data/archive
mv ./data/trade_history.jsonl "./data/archive/trade_history_$(date +%Y%m%d).jsonl"

# Bot will create new file automatically on next trade
```

### Data Cleanup (If needed)

```bash
# Keep only last 30 days (requires jq)
CUTOFF=$(date -v-30d +%s)
jq -c "select(.timestamp >= $CUTOFF)" ./data/trade_history.jsonl > ./data/temp.jsonl
mv ./data/temp.jsonl ./data/trade_history.jsonl
```

## Alerting (Optional Enhancement)

Add this to get notified of issues:

```rust
async fn check_performance_alerts(storage: &TradeStorage) -> anyhow::Result<()> {
    let records = storage.load_all_records().await?;
    let recent: Vec<_> = records.iter().rev().take(100).collect();
    
    if recent.is_empty() {
        return Ok(());
    }
    
    // Calculate recent success rate
    let success_count = recent.iter().filter(|r| r.success).count();
    let success_rate = (success_count as f64 / recent.len() as f64) * 100.0;
    
    // Alert if success rate drops below 50%
    if success_rate < 50.0 {
        tracing::error!("⚠️ ALERT: Success rate dropped to {:.1}%!", success_rate);
        // TODO: Send Discord/Telegram notification
    }
    
    // Alert if average latency > 500ms
    let avg_latency: f64 = recent.iter()
        .map(|r| r.latency_ms as f64)
        .sum::<f64>() / recent.len() as f64;
    
    if avg_latency > 500.0 {
        tracing::warn!("⚠️ High latency detected: {:.0}ms average", avg_latency);
    }
    
    Ok(())
}

// Run every 15 minutes
tokio::spawn(async move {
    let mut ticker = interval(Duration::from_secs(900));
    loop {
        ticker.tick().await;
        check_performance_alerts(&storage).await.ok();
    }
});
```

## Troubleshooting

### "No trade data available yet"
- Bot hasn't executed any trades yet
- Check that bot is running and finding opportunities

### File not found error
- Run bot once to create `./data/trade_history.jsonl`
- Or manually: `mkdir -p ./data && touch ./data/trade_history.jsonl`

### Corrupted records
- Storage service skips bad lines automatically
- Check logs for "Failed to deserialize" warnings
- Manually fix or remove bad lines

### High memory usage during report generation
- Loading millions of records uses RAM
- Implement log rotation (see above)
- Or load in batches for analysis

## Next Steps

1. **Run your bot** - Data collection starts automatically
2. **Let it trade** - Build up some history (10+ trades minimum)
3. **Generate first report** - See your performance metrics
4. **Set up periodic reports** - Monitor performance over time
5. **Add alerting** - Get notified of issues immediately

## Example Report Output

After 150 trades, you'll see something like:

```markdown
# MEV Bot Performance Report

## Executive Summary

- **Total Trades**: 150
- **Successful**: 132 (88.00%)
- **Failed**: 18
- **Average Latency**: 127.50 ms

## Profit Analysis

- **Total Profit**: 2.456789 tokens
- **Average Profit**: 0.018612 tokens per trade
- **Profit Std Dev**: 0.004123 (risk metric)
- **Coefficient of Variation**: 22.15%

### Risk Assessment

✅ **Low Risk**: Consistent profit performance

## Failure Analysis

Top failure reasons:

1. **10** occurrences (55.6%)
   ```
   Slippage tolerance exceeded
   ```

2. **5** occurrences (27.8%)
   ```
   Insufficient liquidity
   ```

3. **3** occurrences (16.7%)
   ```
   RPC timeout
   ```
```

This tells you:
- Bot is performing well (88% success)
- Low execution latency (127ms average)
- Consistent profits (CV 22% = low risk)
- Main issue is slippage (55% of failures)

## Support

If you encounter issues:
1. Check logs for error messages
2. Verify `./data/trade_history.jsonl` exists and is readable
3. Run `cargo test data::` to verify storage is working
4. Review `PHASE3_ANALYTICS_SUMMARY.md` for detailed documentation

---

**Phase 3 Analytics is now running!** 🚀

Your bot will automatically build up a comprehensive performance history.
