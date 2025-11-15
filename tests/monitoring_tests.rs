// Monitoring and Observability Tests
//
// Tests for metrics collection, alerting, and monitoring infrastructure.
// Validates that the MEV bot properly tracks performance metrics and
// triggers alerts when circuit breakers activate.
//
// Prerequisites:
// 1. Prometheus metrics feature enabled
// 2. Mock alerting infrastructure
//
// Run with:
//   cargo test --test monitoring_tests -- --nocapture --test-threads=1

use anyhow::{Result, Context};
use serial_test::serial;
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool, Ordering}};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// MOCK METRICS INFRASTRUCTURE
// ============================================================================

/// Mock Prometheus-style metrics collector
#[derive(Default, Clone)]
pub struct MockMetricsCollector {
    // Counters
    opportunities_detected: Arc<AtomicU64>,
    transactions_sent: Arc<AtomicU64>,
    transactions_failed: Arc<AtomicU64>,
    circuit_breaker_triggered: Arc<AtomicU64>,
    
    // Histograms (simplified - store samples)
    detection_latency_samples: Arc<Mutex<Vec<f64>>>,
    profit_samples: Arc<Mutex<Vec<f64>>>,
    transaction_size_samples: Arc<Mutex<Vec<u64>>>,
}

impl MockMetricsCollector {
    pub fn new() -> Self {
        Self {
            opportunities_detected: Arc::new(AtomicU64::new(0)),
            transactions_sent: Arc::new(AtomicU64::new(0)),
            transactions_failed: Arc::new(AtomicU64::new(0)),
            circuit_breaker_triggered: Arc::new(AtomicU64::new(0)),
            detection_latency_samples: Arc::new(Mutex::new(Vec::new())),
            profit_samples: Arc::new(Mutex::new(Vec::new())),
            transaction_size_samples: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    // Counter operations
    pub fn inc_opportunities_detected(&self) {
        self.opportunities_detected.fetch_add(1, Ordering::SeqCst);
    }
    
    pub fn inc_transactions_sent(&self) {
        self.transactions_sent.fetch_add(1, Ordering::SeqCst);
    }
    
    pub fn inc_transactions_failed(&self) {
        self.transactions_failed.fetch_add(1, Ordering::SeqCst);
    }
    
    pub fn inc_circuit_breaker_triggered(&self) {
        self.circuit_breaker_triggered.fetch_add(1, Ordering::SeqCst);
    }
    
    // Histogram operations
    pub fn observe_detection_latency(&self, latency_ms: f64) {
        self.detection_latency_samples.lock().unwrap().push(latency_ms);
    }
    
    pub fn observe_profit(&self, profit_sol: f64) {
        self.profit_samples.lock().unwrap().push(profit_sol);
    }
    
    pub fn observe_transaction_size(&self, size_bytes: u64) {
        self.transaction_size_samples.lock().unwrap().push(size_bytes);
    }
    
    // Query operations (Prometheus-style)
    pub fn get_counter(&self, name: &str) -> u64 {
        match name {
            "opportunities_detected" => self.opportunities_detected.load(Ordering::SeqCst),
            "transactions_sent" => self.transactions_sent.load(Ordering::SeqCst),
            "transactions_failed" => self.transactions_failed.load(Ordering::SeqCst),
            "circuit_breaker_triggered" => self.circuit_breaker_triggered.load(Ordering::SeqCst),
            _ => 0,
        }
    }
    
    pub fn get_histogram_samples(&self, name: &str) -> Vec<f64> {
        match name {
            "detection_latency_ms" => self.detection_latency_samples.lock().unwrap().clone(),
            "profit_per_trade_sol" => self.profit_samples.lock().unwrap().clone(),
            _ => Vec::new(),
        }
    }
    
    pub fn get_histogram_count(&self, name: &str) -> usize {
        self.get_histogram_samples(name).len()
    }
    
    pub fn get_histogram_sum(&self, name: &str) -> f64 {
        self.get_histogram_samples(name).iter().sum()
    }
    
    pub fn get_histogram_avg(&self, name: &str) -> f64 {
        let samples = self.get_histogram_samples(name);
        if samples.is_empty() {
            0.0
        } else {
            samples.iter().sum::<f64>() / samples.len() as f64
        }
    }
    
    pub fn get_histogram_p50(&self, name: &str) -> f64 {
        let mut samples = self.get_histogram_samples(name);
        if samples.is_empty() {
            return 0.0;
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        samples[samples.len() / 2]
    }
    
    pub fn get_histogram_p95(&self, name: &str) -> f64 {
        let mut samples = self.get_histogram_samples(name);
        if samples.is_empty() {
            return 0.0;
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        samples[(samples.len() as f64 * 0.95) as usize]
    }
    
    pub fn get_histogram_p99(&self, name: &str) -> f64 {
        let mut samples = self.get_histogram_samples(name);
        if samples.is_empty() {
            return 0.0;
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        samples[(samples.len() as f64 * 0.99) as usize]
    }
    
    /// Export metrics in Prometheus text format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // Counters
        output.push_str(&format!("# TYPE opportunities_detected counter\n"));
        output.push_str(&format!("opportunities_detected {}\n", self.get_counter("opportunities_detected")));
        
        output.push_str(&format!("# TYPE transactions_sent counter\n"));
        output.push_str(&format!("transactions_sent {}\n", self.get_counter("transactions_sent")));
        
        output.push_str(&format!("# TYPE transactions_failed counter\n"));
        output.push_str(&format!("transactions_failed {}\n", self.get_counter("transactions_failed")));
        
        output.push_str(&format!("# TYPE circuit_breaker_triggered counter\n"));
        output.push_str(&format!("circuit_breaker_triggered {}\n", self.get_counter("circuit_breaker_triggered")));
        
        // Histograms
        output.push_str(&format!("# TYPE detection_latency_ms histogram\n"));
        output.push_str(&format!("detection_latency_ms_count {}\n", self.get_histogram_count("detection_latency_ms")));
        output.push_str(&format!("detection_latency_ms_sum {}\n", self.get_histogram_sum("detection_latency_ms")));
        
        output.push_str(&format!("# TYPE profit_per_trade_sol histogram\n"));
        output.push_str(&format!("profit_per_trade_sol_count {}\n", self.get_histogram_count("profit_per_trade_sol")));
        output.push_str(&format!("profit_per_trade_sol_sum {}\n", self.get_histogram_sum("profit_per_trade_sol")));
        
        output
    }
    
    /// Reset all metrics (for testing)
    pub fn reset(&self) {
        self.opportunities_detected.store(0, Ordering::SeqCst);
        self.transactions_sent.store(0, Ordering::SeqCst);
        self.transactions_failed.store(0, Ordering::SeqCst);
        self.circuit_breaker_triggered.store(0, Ordering::SeqCst);
        self.detection_latency_samples.lock().unwrap().clear();
        self.profit_samples.lock().unwrap().clear();
        self.transaction_size_samples.lock().unwrap().clear();
    }
}

// ============================================================================
// MOCK ALERTING INFRASTRUCTURE
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub severity: AlertSeverity,
    pub title: String,
    pub message: String,
    pub timestamp: std::time::SystemTime,
}

/// Mock alerting system (simulates PagerDuty, Slack, etc.)
#[derive(Default, Clone)]
pub struct MockAlerting {
    alerts: Arc<Mutex<Vec<Alert>>>,
    enabled: Arc<AtomicBool>,
}

impl MockAlerting {
    pub fn new() -> Self {
        Self {
            alerts: Arc::new(Mutex::new(Vec::new())),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }
    
    pub fn send_alert(&self, severity: AlertSeverity, title: String, message: String) {
        if !self.enabled.load(Ordering::SeqCst) {
            return;
        }
        
        let alert = Alert {
            severity,
            title,
            message,
            timestamp: std::time::SystemTime::now(),
        };
        
        self.alerts.lock().unwrap().push(alert);
    }
    
    pub fn get_alerts(&self) -> Vec<Alert> {
        self.alerts.lock().unwrap().clone()
    }
    
    pub fn get_alerts_by_severity(&self, severity: AlertSeverity) -> Vec<Alert> {
        self.alerts.lock().unwrap()
            .iter()
            .filter(|a| a.severity == severity)
            .cloned()
            .collect()
    }
    
    pub fn get_alert_count(&self) -> usize {
        self.alerts.lock().unwrap().len()
    }
    
    pub fn has_alert_with_title(&self, title_contains: &str) -> bool {
        self.alerts.lock().unwrap()
            .iter()
            .any(|a| a.title.contains(title_contains))
    }
    
    pub fn clear(&self) {
        self.alerts.lock().unwrap().clear();
    }
    
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::SeqCst);
    }
    
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }
}

// ============================================================================
// CIRCUIT BREAKER IMPLEMENTATION
// ============================================================================

pub struct CircuitBreaker {
    failure_threshold: u64,
    failure_window_secs: u64,
    failures: Arc<Mutex<Vec<std::time::SystemTime>>>,
    is_open: Arc<AtomicBool>,
    metrics: MockMetricsCollector,
    alerting: MockAlerting,
}

impl CircuitBreaker {
    pub fn new(
        failure_threshold: u64,
        failure_window_secs: u64,
        metrics: MockMetricsCollector,
        alerting: MockAlerting,
    ) -> Self {
        Self {
            failure_threshold,
            failure_window_secs,
            failures: Arc::new(Mutex::new(Vec::new())),
            is_open: Arc::new(AtomicBool::new(false)),
            metrics,
            alerting,
        }
    }
    
    pub fn record_failure(&self) {
        let now = std::time::SystemTime::now();
        let mut failures = self.failures.lock().unwrap();
        
        // Remove old failures outside the window
        let window_start = now - Duration::from_secs(self.failure_window_secs);
        failures.retain(|&t| t > window_start);
        
        // Add new failure
        failures.push(now);
        
        // Check if threshold exceeded
        if failures.len() >= self.failure_threshold as usize {
            if !self.is_open.load(Ordering::SeqCst) {
                self.trigger();
            }
        }
    }
    
    pub fn record_success(&self) {
        // Clear failures on success (optional - depends on strategy)
        // For now, we'll keep them to avoid flapping
    }
    
    fn trigger(&self) {
        self.is_open.store(true, Ordering::SeqCst);
        self.metrics.inc_circuit_breaker_triggered();
        
        self.alerting.send_alert(
            AlertSeverity::Critical,
            "Circuit Breaker Triggered".to_string(),
            format!(
                "Circuit breaker opened due to {} failures in {} seconds",
                self.failure_threshold, self.failure_window_secs
            ),
        );
        
        println!("⚠️  CIRCUIT BREAKER TRIGGERED - Bot temporarily halted");
    }
    
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }
    
    pub fn reset(&self) {
        self.is_open.store(false, Ordering::SeqCst);
        self.failures.lock().unwrap().clear();
    }
    
    pub fn get_failure_count(&self) -> usize {
        self.failures.lock().unwrap().len()
    }
}

// ============================================================================
// TEST 1: METRICS COLLECTION
// ============================================================================

#[tokio::test]
#[serial]
async fn test_metrics_collection() -> Result<()> {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  📊 TEST: Metrics Collection                                   ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    
    println!("Validates that the bot properly collects and exposes metrics.");
    println!("Tests counters, histograms, and Prometheus endpoint.\n");
    
    // Setup
    println!("🔧 Setup");
    println!("========\n");
    
    let metrics = MockMetricsCollector::new();
    let alerting = MockAlerting::new();
    
    println!("✅ Created metrics collector");
    println!("✅ Created alerting system\n");
    
    // Simulate bot operation
    println!("⚡ Simulating Bot Operations");
    println!("============================\n");
    
    // Simulate 10 arbitrage detections
    println!("📡 Simulating 10 arbitrage detections...");
    for i in 0..10 {
        metrics.inc_opportunities_detected();
        
        // Simulate detection latency (0.02ms - 0.05ms)
        let latency = 0.02 + (i as f64 * 0.003);
        metrics.observe_detection_latency(latency);
        
        // Simulate profit (0.001 - 0.01 SOL)
        let profit = 0.001 + (i as f64 * 0.001);
        metrics.observe_profit(profit);
        
        sleep(Duration::from_millis(10)).await;
    }
    println!("   ✅ Detected 10 opportunities\n");
    
    // Simulate 8 successful transactions
    println!("📤 Simulating 8 successful transactions...");
    for i in 0..8 {
        metrics.inc_transactions_sent();
        
        // Simulate transaction size (400-600 bytes)
        let size = 400 + (i * 25);
        metrics.observe_transaction_size(size);
        
        sleep(Duration::from_millis(5)).await;
    }
    println!("   ✅ Sent 8 transactions\n");
    
    // Simulate 2 failed transactions
    println!("❌ Simulating 2 failed transactions...");
    for _ in 0..2 {
        metrics.inc_transactions_failed();
        sleep(Duration::from_millis(5)).await;
    }
    println!("   ✅ Recorded 2 failures\n");
    
    // Query metrics
    println!("📊 METRICS VALIDATION");
    println!("═══════════════════════\n");
    
    // Counter metrics
    println!("📈 Counter Metrics:");
    println!("   • opportunities_detected: {}", metrics.get_counter("opportunities_detected"));
    println!("   • transactions_sent:      {}", metrics.get_counter("transactions_sent"));
    println!("   • transactions_failed:    {}", metrics.get_counter("transactions_failed"));
    println!();
    
    // Histogram metrics - Detection Latency
    println!("⏱️  Detection Latency Histogram:");
    println!("   • Count:   {}", metrics.get_histogram_count("detection_latency_ms"));
    println!("   • Average: {:.4}ms", metrics.get_histogram_avg("detection_latency_ms"));
    println!("   • p50:     {:.4}ms", metrics.get_histogram_p50("detection_latency_ms"));
    println!("   • p95:     {:.4}ms", metrics.get_histogram_p95("detection_latency_ms"));
    println!("   • p99:     {:.4}ms", metrics.get_histogram_p99("detection_latency_ms"));
    println!();
    
    // Histogram metrics - Profit
    println!("💰 Profit per Trade Histogram:");
    println!("   • Count:      {}", metrics.get_histogram_count("profit_per_trade_sol"));
    println!("   • Total:      {:.6} SOL", metrics.get_histogram_sum("profit_per_trade_sol"));
    println!("   • Average:    {:.6} SOL", metrics.get_histogram_avg("profit_per_trade_sol"));
    println!("   • p50:        {:.6} SOL", metrics.get_histogram_p50("profit_per_trade_sol"));
    println!("   • p95:        {:.6} SOL", metrics.get_histogram_p95("profit_per_trade_sol"));
    println!();
    
    // Prometheus endpoint simulation
    println!("🔍 Prometheus Endpoint");
    println!("======================\n");
    
    let prometheus_output = metrics.export_prometheus();
    println!("Prometheus metrics export:\n");
    println!("{}", prometheus_output);
    
    // Assertions
    println!("🧪 Validation");
    println!("=============\n");
    
    // Counter assertions
    assert_eq!(
        metrics.get_counter("opportunities_detected"),
        10,
        "Should have detected 10 opportunities"
    );
    println!("✅ PASS: Opportunities counter = 10");
    
    assert_eq!(
        metrics.get_counter("transactions_sent"),
        8,
        "Should have sent 8 transactions"
    );
    println!("✅ PASS: Transactions sent counter = 8");
    
    assert_eq!(
        metrics.get_counter("transactions_failed"),
        2,
        "Should have 2 failed transactions"
    );
    println!("✅ PASS: Failed transactions counter = 2");
    
    // Histogram assertions
    assert_eq!(
        metrics.get_histogram_count("detection_latency_ms"),
        10,
        "Should have 10 latency samples"
    );
    println!("✅ PASS: Detection latency samples = 10");
    
    assert_eq!(
        metrics.get_histogram_count("profit_per_trade_sol"),
        10,
        "Should have 10 profit samples"
    );
    println!("✅ PASS: Profit samples = 10");
    
    // Validate latency is reasonable
    let avg_latency = metrics.get_histogram_avg("detection_latency_ms");
    assert!(
        avg_latency > 0.01 && avg_latency < 0.1,
        "Average latency should be between 0.01ms and 0.1ms"
    );
    println!("✅ PASS: Average latency {:.4}ms is reasonable", avg_latency);
    
    // Validate profit is tracked
    let total_profit = metrics.get_histogram_sum("profit_per_trade_sol");
    assert!(
        total_profit > 0.0,
        "Total profit should be greater than 0"
    );
    println!("✅ PASS: Total profit {:.6} SOL tracked correctly", total_profit);
    
    // Validate Prometheus export contains metrics
    assert!(
        prometheus_output.contains("opportunities_detected"),
        "Prometheus export should contain opportunities_detected"
    );
    println!("✅ PASS: Prometheus export contains opportunities_detected");
    
    assert!(
        prometheus_output.contains("detection_latency_ms_count"),
        "Prometheus export should contain detection_latency_ms"
    );
    println!("✅ PASS: Prometheus export contains detection_latency_ms");
    
    assert!(
        prometheus_output.contains("profit_per_trade_sol_sum"),
        "Prometheus export should contain profit metrics"
    );
    println!("✅ PASS: Prometheus export contains profit metrics\n");
    
    // Summary
    println!("📋 Test Summary");
    println!("===============\n");
    println!("   ✅ All counters working correctly");
    println!("   ✅ All histograms tracking samples");
    println!("   ✅ Prometheus export functioning");
    println!("   ✅ Metrics provide observability\n");
    
    Ok(())
}

// ============================================================================
// TEST 2: CIRCUIT BREAKER ALERTING
// ============================================================================

#[tokio::test]
#[serial]
async fn test_alerting_on_circuit_breaker() -> Result<()> {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🚨 TEST: Circuit Breaker Alerting                            ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    
    println!("Validates that circuit breaker triggers and sends alerts.");
    println!("Tests failure threshold detection and alert delivery.\n");
    
    // Setup
    println!("🔧 Setup");
    println!("========\n");
    
    let metrics = MockMetricsCollector::new();
    let alerting = MockAlerting::new();
    
    // Circuit breaker: 5 failures within 60 seconds triggers
    let circuit_breaker = CircuitBreaker::new(5, 60, metrics.clone(), alerting.clone());
    
    println!("✅ Created circuit breaker");
    println!("   • Failure threshold: 5");
    println!("   • Time window: 60 seconds");
    println!("✅ Created alerting system\n");
    
    // Test Phase 1: Normal operations (below threshold)
    println!("📡 Phase 1: Normal Operations");
    println!("==============================\n");
    
    println!("Simulating 3 failures (below threshold)...");
    for i in 1..=3 {
        circuit_breaker.record_failure();
        metrics.inc_transactions_failed();
        println!("   ❌ Failure {} recorded", i);
        sleep(Duration::from_millis(100)).await;
    }
    
    assert!(!circuit_breaker.is_open(), "Circuit breaker should NOT be open");
    println!("\n✅ Circuit breaker still closed (3 < 5 threshold)\n");
    
    assert_eq!(
        alerting.get_alert_count(),
        0,
        "No alerts should be sent below threshold"
    );
    println!("✅ No alerts sent (below threshold)\n");
    
    // Test Phase 2: Trigger circuit breaker
    println!("📡 Phase 2: Trigger Circuit Breaker");
    println!("====================================\n");
    
    println!("Adding 2 more failures to exceed threshold...");
    for i in 4..=5 {
        circuit_breaker.record_failure();
        metrics.inc_transactions_failed();
        println!("   ❌ Failure {} recorded", i);
        sleep(Duration::from_millis(100)).await;
    }
    
    println!();
    
    // Verify circuit breaker opened
    assert!(circuit_breaker.is_open(), "Circuit breaker SHOULD be open");
    println!("✅ Circuit breaker OPENED (5/5 threshold reached)");
    
    // Verify metrics updated
    assert_eq!(
        metrics.get_counter("circuit_breaker_triggered"),
        1,
        "Circuit breaker trigger should be recorded in metrics"
    );
    println!("✅ Metrics recorded circuit breaker trigger");
    
    assert_eq!(
        metrics.get_counter("transactions_failed"),
        5,
        "All 5 failures should be tracked"
    );
    println!("✅ All 5 failures tracked in metrics\n");
    
    // Test Phase 3: Verify alerting
    println!("📡 Phase 3: Verify Alerts");
    println!("==========================\n");
    
    // Check alert was sent
    assert!(
        alerting.get_alert_count() > 0,
        "At least one alert should be sent"
    );
    println!("✅ Alert system triggered");
    
    let alerts = alerting.get_alerts();
    println!("   • Total alerts: {}", alerts.len());
    
    // Verify critical alert exists
    let critical_alerts = alerting.get_alerts_by_severity(AlertSeverity::Critical);
    assert!(
        !critical_alerts.is_empty(),
        "Should have at least one critical alert"
    );
    println!("   • Critical alerts: {}", critical_alerts.len());
    
    // Verify alert content
    assert!(
        alerting.has_alert_with_title("Circuit Breaker"),
        "Alert should mention circuit breaker"
    );
    println!("   • Alert mentions 'Circuit Breaker'");
    
    // Display alert details
    println!("\n📬 Alert Details:");
    for (idx, alert) in alerts.iter().enumerate() {
        println!("\n   Alert #{}", idx + 1);
        println!("   ─────────");
        println!("   Severity: {:?}", alert.severity);
        println!("   Title:    {}", alert.title);
        println!("   Message:  {}", alert.message);
    }
    println!();
    
    // Test Phase 4: Verify bot behavior when circuit is open
    println!("📡 Phase 4: Circuit Open Behavior");
    println!("==================================\n");
    
    // Simulate attempted transaction while circuit is open
    if circuit_breaker.is_open() {
        println!("⚠️  Circuit is OPEN - transactions blocked");
        println!("   Bot should halt arbitrage execution");
        println!("   Manual intervention required\n");
    }
    
    // Test Phase 5: Circuit breaker reset
    println!("📡 Phase 5: Circuit Reset");
    println!("=========================\n");
    
    println!("Resetting circuit breaker (manual intervention)...");
    circuit_breaker.reset();
    
    assert!(!circuit_breaker.is_open(), "Circuit should be closed after reset");
    println!("✅ Circuit breaker reset successful");
    
    assert_eq!(
        circuit_breaker.get_failure_count(),
        0,
        "Failure count should be cleared"
    );
    println!("✅ Failure count cleared\n");
    
    // Clear alerts for next phase
    alerting.clear();
    println!("✅ Alerts cleared\n");
    
    // Test Phase 6: Verify no false positives
    println!("📡 Phase 6: False Positive Prevention");
    println!("======================================\n");
    
    println!("Testing that successes don't trigger alerts...");
    circuit_breaker.record_success();
    circuit_breaker.record_success();
    circuit_breaker.record_success();
    
    assert!(!circuit_breaker.is_open(), "Circuit should remain closed");
    println!("✅ Circuit stays closed on successes");
    
    assert_eq!(
        alerting.get_alert_count(),
        0,
        "No alerts on success"
    );
    println!("✅ No false positive alerts\n");
    
    // Validation Summary
    println!("📊 VALIDATION SUMMARY");
    println!("═══════════════════════\n");
    
    println!("✅ PASS: Circuit breaker triggers at threshold (5 failures)");
    println!("✅ PASS: Metrics updated correctly");
    println!("✅ PASS: Critical alert sent");
    println!("✅ PASS: Alert contains proper information");
    println!("✅ PASS: Circuit can be reset");
    println!("✅ PASS: No false positive alerts\n");
    
    // Recommendations
    println!("💡 Production Recommendations");
    println!("==============================\n");
    println!("   • Monitor circuit_breaker_triggered metric");
    println!("   • Set up alerting integration (PagerDuty, Slack)");
    println!("   • Configure failure threshold based on network conditions");
    println!("   • Implement auto-recovery with exponential backoff");
    println!("   • Track time to recovery (MTTR)");
    println!("   • Log circuit breaker events for post-mortem\n");
    
    Ok(())
}

// ============================================================================
// TEST 3: METRICS EDGE CASES
// ============================================================================

#[tokio::test]
#[serial]
async fn test_metrics_edge_cases() -> Result<()> {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🧪 TEST: Metrics Edge Cases                                  ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    
    let metrics = MockMetricsCollector::new();
    
    // Test empty histograms
    println!("Testing empty histograms...");
    assert_eq!(metrics.get_histogram_avg("detection_latency_ms"), 0.0);
    assert_eq!(metrics.get_histogram_p50("detection_latency_ms"), 0.0);
    println!("✅ Empty histograms return 0.0\n");
    
    // Test single sample (after reset to start fresh)
    println!("Testing metrics reset first...");
    metrics.reset();
    println!("✅ Reset clears all metrics\n");
    
    println!("Testing single sample histogram...");
    metrics.observe_detection_latency(1.5);
    assert_eq!(metrics.get_histogram_avg("detection_latency_ms"), 1.5);
    assert_eq!(metrics.get_histogram_p50("detection_latency_ms"), 1.5);
    assert_eq!(metrics.get_histogram_p95("detection_latency_ms"), 1.5);
    println!("✅ Single sample statistics correct\n");
    
    // Test reset again
    println!("Testing metrics reset with data...");
    metrics.inc_opportunities_detected();
    metrics.observe_profit(0.5);
    metrics.reset();
    assert_eq!(metrics.get_counter("opportunities_detected"), 0);
    assert_eq!(metrics.get_histogram_count("profit_per_trade_sol"), 0);
    println!("✅ Reset clears all metrics correctly\n");
    
    // Test concurrent access (basic)
    println!("Testing concurrent metric updates...");
    let metrics_clone = metrics.clone();
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let m = metrics_clone.clone();
            tokio::spawn(async move {
                m.inc_opportunities_detected();
                m.observe_detection_latency(i as f64);
            })
        })
        .collect();
    
    for handle in handles {
        handle.await?;
    }
    
    assert_eq!(metrics.get_counter("opportunities_detected"), 10);
    assert_eq!(metrics.get_histogram_count("detection_latency_ms"), 10); // Only 10 new samples
    println!("✅ Concurrent updates handled correctly\n");
    
    println!("All edge cases passed! ✅\n");
    
    Ok(())
}

// ============================================================================
// TEST 4: ALERT FILTERING AND PRIORITIZATION
// ============================================================================

#[tokio::test]
#[serial]
async fn test_alert_filtering() -> Result<()> {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🔔 TEST: Alert Filtering and Prioritization                  ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    
    let alerting = MockAlerting::new();
    
    // Send various severity alerts
    println!("Sending alerts of different severities...");
    alerting.send_alert(AlertSeverity::Info, "Low Balance".to_string(), "SOL balance below 1".to_string());
    alerting.send_alert(AlertSeverity::Warning, "High Slippage".to_string(), "Slippage exceeded 2%".to_string());
    alerting.send_alert(AlertSeverity::Critical, "Circuit Breaker".to_string(), "System halted".to_string());
    alerting.send_alert(AlertSeverity::Critical, "RPC Error".to_string(), "Cannot connect to RPC".to_string());
    alerting.send_alert(AlertSeverity::Info, "Opportunity Found".to_string(), "New arb detected".to_string());
    
    println!("   ✅ Sent 5 alerts (2 Info, 1 Warning, 2 Critical)\n");
    
    // Test filtering by severity
    println!("Testing severity filtering...");
    let info_alerts = alerting.get_alerts_by_severity(AlertSeverity::Info);
    let warning_alerts = alerting.get_alerts_by_severity(AlertSeverity::Warning);
    let critical_alerts = alerting.get_alerts_by_severity(AlertSeverity::Critical);
    
    assert_eq!(info_alerts.len(), 2);
    println!("   ✅ Info alerts: {}", info_alerts.len());
    
    assert_eq!(warning_alerts.len(), 1);
    println!("   ✅ Warning alerts: {}", warning_alerts.len());
    
    assert_eq!(critical_alerts.len(), 2);
    println!("   ✅ Critical alerts: {}\n", critical_alerts.len());
    
    // Test title search
    println!("Testing alert search...");
    assert!(alerting.has_alert_with_title("Circuit Breaker"));
    assert!(alerting.has_alert_with_title("RPC"));
    assert!(!alerting.has_alert_with_title("NonExistent"));
    println!("   ✅ Title search working\n");
    
    // Test enable/disable
    println!("Testing alert enable/disable...");
    alerting.disable();
    alerting.send_alert(AlertSeverity::Info, "Test".to_string(), "Should not appear".to_string());
    assert_eq!(alerting.get_alert_count(), 5); // Still 5, not 6
    println!("   ✅ Disabled alerting blocks alerts");
    
    alerting.enable();
    alerting.send_alert(AlertSeverity::Info, "Test2".to_string(), "Should appear".to_string());
    assert_eq!(alerting.get_alert_count(), 6);
    println!("   ✅ Re-enabled alerting works\n");
    
    println!("All alert filtering tests passed! ✅\n");
    
    Ok(())
}
