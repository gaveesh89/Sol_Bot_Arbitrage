# 📊 Monitoring Tests Guide

Comprehensive guide for monitoring and observability tests in the Solana MEV Bot.

## 📋 Overview

The monitoring tests validate that the bot properly tracks performance metrics, triggers alerts when circuit breakers activate, and provides observability into bot operations.

**File:** `tests/monitoring_tests.rs`

**Test Coverage:**
- ✅ Metrics collection (counters, histograms)
- ✅ Prometheus endpoint export
- ✅ Circuit breaker triggering
- ✅ Alert delivery system
- ✅ Metrics edge cases
- ✅ Alert filtering and prioritization

---

## 🚀 Running the Tests

### Run All Monitoring Tests
```bash
cargo test --test monitoring_tests -- --nocapture --test-threads=1
```

### Run Individual Tests

**Metrics Collection:**
```bash
cargo test --test monitoring_tests test_metrics_collection -- --nocapture
```

**Circuit Breaker Alerting:**
```bash
cargo test --test monitoring_tests test_alerting_on_circuit_breaker -- --nocapture
```

**Metrics Edge Cases:**
```bash
cargo test --test monitoring_tests test_metrics_edge_cases -- --nocapture
```

**Alert Filtering:**
```bash
cargo test --test monitoring_tests test_alert_filtering -- --nocapture
```

---

## 📊 Test 1: Metrics Collection

**Purpose:** Validates that the bot properly collects and exposes performance metrics.

### What It Tests

1. **Counter Metrics:**
   - `opportunities_detected` - Total arbitrage opportunities found
   - `transactions_sent` - Total transactions submitted
   - `transactions_failed` - Total failed transactions
   - `circuit_breaker_triggered` - Circuit breaker activation count

2. **Histogram Metrics:**
   - `detection_latency_ms` - Time to detect arbitrage opportunities
   - `profit_per_trade_sol` - Profit per executed trade
   - `transaction_size_bytes` - Transaction payload size

3. **Prometheus Endpoint:**
   - Validates Prometheus text format export
   - Verifies all metrics are present
   - Checks histogram summaries (count, sum)

### Example Output

```
📊 METRICS VALIDATION
═══════════════════════

📈 Counter Metrics:
   • opportunities_detected: 10
   • transactions_sent:      8
   • transactions_failed:    2

⏱️  Detection Latency Histogram:
   • Count:   10
   • Average: 0.0335ms
   • p50:     0.0350ms
   • p95:     0.0470ms
   • p99:     0.0470ms

💰 Profit per Trade Histogram:
   • Count:      10
   • Total:      0.055000 SOL
   • Average:    0.005500 SOL
   • p50:        0.006000 SOL
   • p95:        0.010000 SOL

🔍 Prometheus Endpoint
======================

# TYPE opportunities_detected counter
opportunities_detected 10
# TYPE transactions_sent counter
transactions_sent 8
# TYPE detection_latency_ms histogram
detection_latency_ms_count 10
detection_latency_ms_sum 0.335
```

### Success Criteria

✅ All counters increment correctly  
✅ Histograms track samples accurately  
✅ Statistics (avg, p50, p95, p99) calculated correctly  
✅ Prometheus export contains all metrics  
✅ Metrics provide observability into bot operations  

---

## 🚨 Test 2: Circuit Breaker Alerting

**Purpose:** Validates that the circuit breaker triggers at failure threshold and sends critical alerts.

### What It Tests

1. **Normal Operations (Below Threshold):**
   - Circuit stays closed with 3/5 failures
   - No alerts sent below threshold
   - Metrics track failures correctly

2. **Circuit Breaker Triggering:**
   - Opens at exactly 5/5 failure threshold
   - Blocks subsequent operations
   - Records trigger in metrics

3. **Alert Delivery:**
   - Sends critical severity alert
   - Alert contains proper title and message
   - Alert timestamp recorded

4. **Circuit Reset:**
   - Manual reset closes circuit
   - Failure count cleared
   - Ready for normal operations

5. **False Positive Prevention:**
   - Success operations don't trigger alerts
   - Circuit stays closed on successes

### Example Output

```
📡 Phase 2: Trigger Circuit Breaker
====================================

Adding 2 more failures to exceed threshold...
   ❌ Failure 4 recorded
⚠️  CIRCUIT BREAKER TRIGGERED - Bot temporarily halted
   ❌ Failure 5 recorded

✅ Circuit breaker OPENED (5/5 threshold reached)
✅ Metrics recorded circuit breaker trigger

📡 Phase 3: Verify Alerts
==========================

✅ Alert system triggered
   • Total alerts: 1
   • Critical alerts: 1

📬 Alert Details:

   Alert #1
   ─────────
   Severity: Critical
   Title:    Circuit Breaker Triggered
   Message:  Circuit breaker opened due to 5 failures in 60 seconds
```

### Success Criteria

✅ Circuit opens at exact threshold (5 failures)  
✅ Metrics updated (circuit_breaker_triggered counter)  
✅ Critical alert sent with proper information  
✅ Circuit can be manually reset  
✅ No false positive alerts on success  

---

## 🧪 Test 3: Metrics Edge Cases

**Purpose:** Validates metrics behavior in edge cases and concurrent scenarios.

### What It Tests

1. **Empty Histograms:**
   - Returns 0.0 for avg, p50, p95, p99 when no samples
   - Doesn't crash on empty data

2. **Single Sample:**
   - All percentiles equal the single value
   - Statistics calculated correctly

3. **Metrics Reset:**
   - All counters reset to 0
   - All histogram samples cleared
   - Clean slate for new data

4. **Concurrent Updates:**
   - Multiple threads updating metrics simultaneously
   - No data races or corruption
   - All updates counted correctly

### Success Criteria

✅ Empty histograms return 0.0 (no crashes)  
✅ Single sample statistics correct  
✅ Reset clears all metrics  
✅ Concurrent updates handled safely  

---

## 🔔 Test 4: Alert Filtering and Prioritization

**Purpose:** Validates alert system can filter by severity and search by title.

### What It Tests

1. **Severity Levels:**
   - Info alerts for non-critical events
   - Warning alerts for concerning conditions
   - Critical alerts for system failures

2. **Alert Filtering:**
   - Filter by severity (Info, Warning, Critical)
   - Count alerts by severity
   - Retrieve specific alert types

3. **Alert Search:**
   - Search by title substring
   - Find specific alerts by keywords
   - Validate alert presence

4. **Enable/Disable:**
   - Disable alerting (maintenance mode)
   - No alerts sent when disabled
   - Re-enable and resume alerting

### Example Output

```
Testing severity filtering...
   ✅ Info alerts: 2
   ✅ Warning alerts: 1
   ✅ Critical alerts: 2

Testing alert search...
   ✅ Title search working

Testing alert enable/disable...
   ✅ Disabled alerting blocks alerts
   ✅ Re-enabled alerting works
```

### Success Criteria

✅ Alerts categorized by severity correctly  
✅ Filtering returns correct subsets  
✅ Title search finds matching alerts  
✅ Enable/disable functionality works  

---

## 🏗️ Mock Infrastructure

The tests use mock implementations to avoid external dependencies:

### MockMetricsCollector

Simulates Prometheus-style metrics without requiring actual Prometheus server.

**Counters:**
- `inc_opportunities_detected()`
- `inc_transactions_sent()`
- `inc_transactions_failed()`
- `inc_circuit_breaker_triggered()`

**Histograms:**
- `observe_detection_latency(ms: f64)`
- `observe_profit(sol: f64)`
- `observe_transaction_size(bytes: u64)`

**Queries:**
- `get_counter(name: &str) -> u64`
- `get_histogram_avg(name: &str) -> f64`
- `get_histogram_p50(name: &str) -> f64`
- `get_histogram_p95(name: &str) -> f64`
- `export_prometheus() -> String`

### MockAlerting

Simulates PagerDuty/Slack alerting without actual webhooks.

**Send Alerts:**
- `send_alert(severity: AlertSeverity, title: String, message: String)`

**Query Alerts:**
- `get_alerts() -> Vec<Alert>`
- `get_alerts_by_severity(severity: AlertSeverity) -> Vec<Alert>`
- `has_alert_with_title(contains: &str) -> bool`
- `get_alert_count() -> usize`

**Control:**
- `enable()` - Resume alerting
- `disable()` - Pause alerting (maintenance mode)
- `clear()` - Remove all alerts

### CircuitBreaker

Production-ready circuit breaker implementation.

**Configuration:**
- `failure_threshold: u64` - Number of failures to trigger (e.g., 5)
- `failure_window_secs: u64` - Time window for counting failures (e.g., 60)

**Operations:**
- `record_failure()` - Track failed transaction
- `record_success()` - Track successful transaction
- `is_open() -> bool` - Check if circuit is triggered
- `reset()` - Manual reset (after incident resolution)

**Behavior:**
- Tracks failures within sliding time window
- Triggers at threshold (e.g., 5 failures in 60 seconds)
- Sends critical alert when triggered
- Updates metrics (circuit_breaker_triggered counter)
- Blocks operations until manual reset

---

## 📈 Production Integration

### Step 1: Enable Prometheus in Cargo.toml

The `prometheus` dependency is already configured as an optional feature:

```toml
[dependencies]
prometheus = { version = "0.13", optional = true }

[features]
metrics = ["prometheus"]
```

Build with metrics:
```bash
cargo build --release --features metrics
```

### Step 2: Expose Metrics Endpoint

Add to your main bot code:

```rust
use prometheus::{Encoder, TextEncoder, Registry};
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};

// Create global registry
lazy_static::lazy_static! {
    static ref REGISTRY: Registry = Registry::new();
    
    // Counters
    static ref OPPORTUNITIES_DETECTED: IntCounter = 
        IntCounter::new("opportunities_detected", "Total arbitrage opportunities")
            .expect("metric creation");
    
    static ref TRANSACTIONS_SENT: IntCounter = 
        IntCounter::new("transactions_sent", "Total transactions sent")
            .expect("metric creation");
    
    // Histograms
    static ref DETECTION_LATENCY: Histogram = 
        Histogram::new(histogram_opts!(
            "detection_latency_ms",
            "Detection latency in milliseconds"
        )).expect("metric creation");
}

// Register metrics
fn register_metrics() {
    REGISTRY.register(Box::new(OPPORTUNITIES_DETECTED.clone())).unwrap();
    REGISTRY.register(Box::new(TRANSACTIONS_SENT.clone())).unwrap();
    REGISTRY.register(Box::new(DETECTION_LATENCY.clone())).unwrap();
}

// Serve metrics on :9090/metrics
async fn serve_metrics() {
    let addr = ([0, 0, 0, 0], 9090).into();
    
    let make_svc = make_service_fn(|_| async {
        Ok::<_, hyper::Error>(service_fn(metrics_handler))
    });
    
    Server::bind(&addr)
        .serve(make_svc)
        .await
        .unwrap();
}

async fn metrics_handler(_req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    
    Ok(Response::new(Body::from(buffer)))
}
```

### Step 3: Query Prometheus Endpoint

```bash
# Query metrics
curl http://localhost:9090/metrics

# Example output:
# opportunities_detected 1523
# transactions_sent 1498
# detection_latency_ms_sum 45.67
# detection_latency_ms_count 1523
```

### Step 4: Set Up Grafana Dashboards

**Example Dashboard Panels:**

1. **Arbitrage Detection Rate**
   - Query: `rate(opportunities_detected[1m])`
   - Visualization: Graph
   - Shows opportunities per second

2. **Transaction Success Rate**
   - Query: `rate(transactions_sent[1m]) / rate(opportunities_detected[1m])`
   - Visualization: Gauge (0-100%)
   - Shows conversion rate

3. **Detection Latency (p95)**
   - Query: `histogram_quantile(0.95, detection_latency_ms_bucket)`
   - Visualization: Graph
   - Shows 95th percentile latency

4. **Circuit Breaker Status**
   - Query: `circuit_breaker_triggered`
   - Visualization: Stat panel
   - Shows total circuit breaker activations

### Step 5: Configure Alerting

**Prometheus AlertManager Rules:**

```yaml
groups:
  - name: mev_bot_alerts
    interval: 10s
    rules:
      # Alert on circuit breaker
      - alert: CircuitBreakerTriggered
        expr: increase(circuit_breaker_triggered[5m]) > 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "MEV Bot Circuit Breaker Triggered"
          description: "Circuit breaker activated - bot halted"
      
      # Alert on high failure rate
      - alert: HighTransactionFailureRate
        expr: rate(transactions_failed[5m]) / rate(transactions_sent[5m]) > 0.1
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High Transaction Failure Rate"
          description: "{{ $value | humanizePercentage }} of transactions failing"
      
      # Alert on slow detection
      - alert: SlowArbitrageDetection
        expr: histogram_quantile(0.95, detection_latency_ms_bucket) > 100
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Slow Arbitrage Detection"
          description: "p95 detection latency: {{ $value }}ms (target: <100ms)"
```

**PagerDuty Integration:**

```rust
use reqwest::Client;

async fn send_pagerduty_alert(title: &str, description: &str) -> Result<()> {
    let client = Client::new();
    
    let payload = serde_json::json!({
        "routing_key": std::env::var("PAGERDUTY_ROUTING_KEY")?,
        "event_action": "trigger",
        "payload": {
            "summary": title,
            "severity": "critical",
            "source": "mev-bot",
            "custom_details": {
                "description": description
            }
        }
    });
    
    client.post("https://events.pagerduty.com/v2/enqueue")
        .json(&payload)
        .send()
        .await?;
    
    Ok(())
}
```

**Slack Integration:**

```rust
async fn send_slack_alert(title: &str, message: &str) -> Result<()> {
    let client = Client::new();
    
    let payload = serde_json::json!({
        "text": format!("🚨 *{}*\n{}", title, message),
        "username": "MEV Bot Monitor",
        "icon_emoji": ":chart_with_upwards_trend:"
    });
    
    client.post(&std::env::var("SLACK_WEBHOOK_URL")?)
        .json(&payload)
        .send()
        .await?;
    
    Ok(())
}
```

---

## 🎯 Best Practices

### 1. Monitor Critical Metrics

**Must-Monitor Metrics:**
- `opportunities_detected` - Validate bot is finding opportunities
- `transactions_sent` - Validate bot is executing trades
- `transactions_failed` - Monitor failure rate
- `circuit_breaker_triggered` - Critical system events
- `detection_latency_ms` (p95, p99) - MEV competitiveness
- `profit_per_trade_sol` - Revenue tracking

### 2. Set Up Alerts

**Critical Alerts:**
- Circuit breaker triggered (immediate PagerDuty)
- RPC connection lost (immediate Slack)
- High transaction failure rate >10% (warning)

**Warning Alerts:**
- Detection latency p95 >100ms (MEV competitiveness at risk)
- Low SOL balance <1 SOL (refill needed)
- High slippage >2% (market conditions poor)

### 3. Dashboard Design

**Real-Time Dashboard:**
- Update every 5-10 seconds
- Show last 1 hour of data
- Include key metrics (detection rate, success rate, latency)

**Daily Summary Dashboard:**
- Total opportunities detected (24h)
- Total profit (24h)
- Average detection latency
- Circuit breaker events
- Failure analysis

### 4. Incident Response

**When Circuit Breaker Triggers:**

1. **Investigate** - Check logs for root cause
2. **Validate Fix** - Ensure issue resolved
3. **Reset Circuit** - Manual intervention required
4. **Monitor** - Watch for recurrence
5. **Post-Mortem** - Document incident and prevention

### 5. Performance Baselines

Establish baselines from test results:

**Detection Latency:**
- Target: <100ms (MEV competitive)
- Good: <50ms
- Excellent: <10ms

**Transaction Building:**
- Target: <50ms
- Good: <20ms
- Excellent: <5ms

**End-to-End:**
- Target: <200ms
- Good: <100ms
- Excellent: <50ms

---

## 📊 Metrics Reference

### Counter Metrics

| Metric | Type | Description | Target |
|--------|------|-------------|--------|
| `opportunities_detected` | Counter | Total arbitrage opportunities found | >100/hour |
| `transactions_sent` | Counter | Total transactions submitted | >80/hour |
| `transactions_failed` | Counter | Total failed transactions | <10/hour |
| `circuit_breaker_triggered` | Counter | Circuit breaker activations | 0 |

### Histogram Metrics

| Metric | Type | Description | Target |
|--------|------|-------------|--------|
| `detection_latency_ms` | Histogram | Detection time (ms) | p95 <100ms |
| `profit_per_trade_sol` | Histogram | Profit per trade (SOL) | avg >0.001 |
| `transaction_size_bytes` | Histogram | Transaction payload size | avg <1000 |

### Histogram Buckets (Recommended)

```rust
// Detection latency buckets (ms)
vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]

// Profit buckets (SOL)
vec![0.0001, 0.001, 0.01, 0.1, 1.0, 10.0]

// Transaction size buckets (bytes)
vec![100, 250, 500, 750, 1000, 1500, 2000]
```

---

## 🔧 Troubleshooting

### Issue: Metrics Not Updating

**Symptoms:**
- Counters stuck at 0
- Histograms empty
- Prometheus endpoint shows no data

**Solutions:**
1. Verify metrics are being called in code:
   ```rust
   OPPORTUNITIES_DETECTED.inc();
   DETECTION_LATENCY.observe(latency_ms);
   ```

2. Check metrics are registered:
   ```rust
   REGISTRY.register(Box::new(OPPORTUNITIES_DETECTED.clone()))?;
   ```

3. Verify Prometheus endpoint is accessible:
   ```bash
   curl http://localhost:9090/metrics
   ```

### Issue: Alerts Not Firing

**Symptoms:**
- Circuit breaker triggers but no alert received
- PagerDuty/Slack silent

**Solutions:**
1. Check alerting is enabled:
   ```rust
   alerting.enable();
   ```

2. Verify webhook URLs are correct:
   ```bash
   echo $PAGERDUTY_ROUTING_KEY
   echo $SLACK_WEBHOOK_URL
   ```

3. Test alert delivery manually:
   ```rust
   alerting.send_alert(
       AlertSeverity::Critical,
       "Test Alert".to_string(),
       "Testing alert delivery".to_string()
   );
   ```

### Issue: Circuit Breaker False Positives

**Symptoms:**
- Circuit triggers on temporary network issues
- Too sensitive to transient failures

**Solutions:**
1. Increase failure threshold:
   ```rust
   let circuit_breaker = CircuitBreaker::new(
       10,  // Was 5, now 10 failures
       60,  // Still 60 second window
       metrics,
       alerting
   );
   ```

2. Increase time window:
   ```rust
   let circuit_breaker = CircuitBreaker::new(
       5,   // 5 failures
       120, // Was 60s, now 120s window
       metrics,
       alerting
   );
   ```

3. Distinguish transient vs persistent failures:
   ```rust
   if is_network_error(&error) {
       // Don't count transient network issues
   } else {
       circuit_breaker.record_failure();
   }
   ```

### Issue: High Memory Usage

**Symptoms:**
- Histogram samples growing unbounded
- Memory leak in metrics

**Solutions:**
1. Limit histogram sample retention:
   ```rust
   // Keep only last 1000 samples
   if samples.len() > 1000 {
       samples.drain(0..samples.len() - 1000);
   }
   ```

2. Periodically reset histograms:
   ```rust
   // Reset every hour
   if last_reset.elapsed() > Duration::from_secs(3600) {
       metrics.reset_histograms();
       last_reset = Instant::now();
   }
   ```

---

## 📚 Additional Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Dashboards](https://grafana.com/docs/grafana/latest/dashboards/)
- [PagerDuty Events API](https://developer.pagerduty.com/docs/events-api-v2/overview/)
- [Slack Incoming Webhooks](https://api.slack.com/messaging/webhooks)
- [Circuit Breaker Pattern](https://martinfowler.com/bliki/CircuitBreaker.html)

---

## 📝 Summary

The monitoring tests validate:

✅ **Metrics Collection** - Counters, histograms, Prometheus export  
✅ **Circuit Breaker** - Failure threshold, alerting, reset  
✅ **Edge Cases** - Empty data, concurrent access, reset  
✅ **Alert Filtering** - Severity levels, search, enable/disable  

All tests pass with comprehensive output showing:
- Real-time metrics updates
- Circuit breaker triggering at exact threshold
- Critical alert delivery
- Prometheus-compatible export format

The mock infrastructure allows testing without external dependencies while providing production-ready implementations for actual deployment.

**Next Steps:**
1. Enable Prometheus feature in production build
2. Set up Grafana dashboards
3. Configure AlertManager rules
4. Integrate PagerDuty/Slack webhooks
5. Establish performance baselines
6. Monitor and iterate

🎯 **Goal:** Zero-downtime, fully observable MEV bot with proactive alerting!
