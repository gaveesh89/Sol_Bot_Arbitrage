use crate::data::{TradeRecord, TradeStorage};
use anyhow::Result;
use std::collections::HashMap;

/// Performance metrics calculated from trade history
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub total_trades: usize,
    pub successful_trades: usize,
    pub failed_trades: usize,
    pub success_rate: f64,
    pub total_profit: f64,
    pub average_profit: f64,
    pub profit_std_dev: f64,
    pub average_latency_ms: f64,
    pub top_failures: Vec<(String, usize)>,
}

/// Generate a comprehensive performance report from trade history
pub async fn generate_report(storage: &TradeStorage) -> Result<String> {
    let records = storage.load_all_records().await?;
    
    if records.is_empty() {
        return Ok("# Performance Report\n\nNo trade data available yet.\n".to_string());
    }

    let metrics = calculate_metrics(&records);
    let report = format_report(&metrics);
    
    Ok(report)
}

/// Calculate performance metrics from trade records
fn calculate_metrics(records: &[TradeRecord]) -> PerformanceMetrics {
    let total_trades = records.len();
    let successful_trades = records.iter().filter(|r| r.success).count();
    let failed_trades = total_trades - successful_trades;
    
    let success_rate = if total_trades > 0 {
        (successful_trades as f64 / total_trades as f64) * 100.0
    } else {
        0.0
    };

    // Calculate profit statistics (only from successful trades)
    let profits: Vec<f64> = records
        .iter()
        .filter(|r| r.success)
        .map(|r| r.profit_amount as f64)
        .collect();

    let total_profit: f64 = profits.iter().sum();
    let average_profit = if !profits.is_empty() {
        total_profit / profits.len() as f64
    } else {
        0.0
    };

    // Calculate standard deviation
    let profit_std_dev = if profits.len() > 1 {
        let variance: f64 = profits
            .iter()
            .map(|p| {
                let diff = p - average_profit;
                diff * diff
            })
            .sum::<f64>()
            / (profits.len() - 1) as f64;
        variance.sqrt()
    } else {
        0.0
    };

    // Calculate average latency
    let average_latency_ms = if total_trades > 0 {
        records.iter().map(|r| r.latency_ms as f64).sum::<f64>() / total_trades as f64
    } else {
        0.0
    };

    // Analyze failures - group by error message
    let mut failure_counts: HashMap<String, usize> = HashMap::new();
    for record in records.iter().filter(|r| !r.success) {
        if let Some(ref error) = record.error_message {
            *failure_counts.entry(error.clone()).or_insert(0) += 1;
        }
    }

    // Get top 3 failures
    let mut top_failures: Vec<(String, usize)> = failure_counts.into_iter().collect();
    top_failures.sort_by(|a, b| b.1.cmp(&a.1));
    top_failures.truncate(3);

    PerformanceMetrics {
        total_trades,
        successful_trades,
        failed_trades,
        success_rate,
        total_profit,
        average_profit,
        profit_std_dev,
        average_latency_ms,
        top_failures,
    }
}

/// Format metrics into a readable Markdown report
fn format_report(metrics: &PerformanceMetrics) -> String {
    let mut report = String::new();
    
    report.push_str("# MEV Bot Performance Report\n\n");
    report.push_str("## Executive Summary\n\n");
    report.push_str(&format!("- **Total Trades**: {}\n", metrics.total_trades));
    report.push_str(&format!("- **Successful**: {} ({:.2}%)\n", 
        metrics.successful_trades, metrics.success_rate));
    report.push_str(&format!("- **Failed**: {}\n", metrics.failed_trades));
    report.push_str(&format!("- **Average Latency**: {:.2} ms\n\n", metrics.average_latency_ms));
    
    if metrics.successful_trades > 0 {
        report.push_str("## Profit Analysis\n\n");
        report.push_str(&format!("- **Total Profit**: {:.6} tokens\n", metrics.total_profit));
        report.push_str(&format!("- **Average Profit**: {:.6} tokens per trade\n", metrics.average_profit));
        report.push_str(&format!("- **Profit Std Dev**: {:.6} (risk metric)\n", metrics.profit_std_dev));
        
        let coefficient_of_variation = if metrics.average_profit != 0.0 {
            (metrics.profit_std_dev / metrics.average_profit) * 100.0
        } else {
            0.0
        };
        report.push_str(&format!("- **Coefficient of Variation**: {:.2}%\n\n", coefficient_of_variation));
        
        // Risk assessment
        report.push_str("### Risk Assessment\n\n");
        if coefficient_of_variation < 50.0 {
            report.push_str("✅ **Low Risk**: Consistent profit performance\n\n");
        } else if coefficient_of_variation < 100.0 {
            report.push_str("⚠️  **Medium Risk**: Moderate profit variability\n\n");
        } else {
            report.push_str("⚠️  **High Risk**: High profit variability - strategy may need adjustment\n\n");
        }
    }
    
    if !metrics.top_failures.is_empty() {
        report.push_str("## Failure Analysis\n\n");
        report.push_str("Top failure reasons:\n\n");
        for (i, (error, count)) in metrics.top_failures.iter().enumerate() {
            let percentage = (*count as f64 / metrics.failed_trades as f64) * 100.0;
            report.push_str(&format!("{}. **{}** occurrences ({:.1}%)\n   ```\n   {}\n   ```\n\n", 
                i + 1, count, percentage, error));
        }
    }
    
    report.push_str("---\n\n");
    report.push_str(&format!("*Report generated at: {}*\n", chrono::Utc::now().to_rfc3339()));
    
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TradeRecord;

    #[test]
    fn test_calculate_metrics_empty() {
        let records: Vec<TradeRecord> = vec![];
        let metrics = calculate_metrics(&records);
        
        assert_eq!(metrics.total_trades, 0);
        assert_eq!(metrics.successful_trades, 0);
        assert_eq!(metrics.failed_trades, 0);
        assert_eq!(metrics.success_rate, 0.0);
    }

    #[test]
    fn test_calculate_metrics_all_success() {
        let timestamp = chrono::Utc::now().timestamp();
        let records = vec![
            TradeRecord::success(
                timestamp,
                "sig1".to_string(),
                "mint1".to_string(),
                100,
                95,
                50,
                "mainnet-fork".to_string(),
            ),
            TradeRecord::success(
                timestamp,
                "sig2".to_string(),
                "mint1".to_string(),
                200,
                190,
                60,
                "mainnet-fork".to_string(),
            ),
        ];
        
        let metrics = calculate_metrics(&records);
        
        assert_eq!(metrics.total_trades, 2);
        assert_eq!(metrics.successful_trades, 2);
        assert_eq!(metrics.failed_trades, 0);
        assert_eq!(metrics.success_rate, 100.0);
        assert_eq!(metrics.total_profit, 300.0);
        assert_eq!(metrics.average_profit, 150.0);
        assert_eq!(metrics.average_latency_ms, 55.0);
    }

    #[test]
    fn test_calculate_metrics_mixed() {
        let timestamp = chrono::Utc::now().timestamp();
        let records = vec![
            TradeRecord::success(
                timestamp,
                "sig1".to_string(),
                "mint1".to_string(),
                100,
                95,
                50,
                "mainnet-fork".to_string(),
            ),
            TradeRecord::failure(
                timestamp,
                "mint1".to_string(),
                95,
                60,
                "mainnet-fork".to_string(),
                "Slippage exceeded".to_string(),
            ),
        ];
        
        let metrics = calculate_metrics(&records);
        
        assert_eq!(metrics.total_trades, 2);
        assert_eq!(metrics.successful_trades, 1);
        assert_eq!(metrics.failed_trades, 1);
        assert_eq!(metrics.success_rate, 50.0);
        assert_eq!(metrics.total_profit, 100.0);
        assert_eq!(metrics.top_failures.len(), 1);
        assert_eq!(metrics.top_failures[0].0, "Slippage exceeded");
        assert_eq!(metrics.top_failures[0].1, 1);
    }

    #[test]
    fn test_format_report() {
        let metrics = PerformanceMetrics {
            total_trades: 10,
            successful_trades: 8,
            failed_trades: 2,
            success_rate: 80.0,
            total_profit: 1000.0,
            average_profit: 125.0,
            profit_std_dev: 25.0,
            average_latency_ms: 55.5,
            top_failures: vec![("Slippage exceeded".to_string(), 2)],
        };
        
        let report = format_report(&metrics);
        
        assert!(report.contains("# MEV Bot Performance Report"));
        assert!(report.contains("Total Trades"));
        assert!(report.contains("80.00%"));
        assert!(report.contains("Profit Analysis"));
        assert!(report.contains("Failure Analysis"));
        assert!(report.contains("Slippage exceeded"));
    }
}
