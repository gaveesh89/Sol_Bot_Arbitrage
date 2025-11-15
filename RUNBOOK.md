# 📘 Solana MEV Bot - Operations Runbook

**Version:** 1.0  
**Last Updated:** November 15, 2025  
**Target Audience:** On-call Engineers, SRE Team

---

## 🎯 Purpose

This runbook provides step-by-step procedures for operating, monitoring, and troubleshooting the Solana MEV Arbitrage Bot in production. Follow these procedures during normal operations and incident response.

---

## 📞 Emergency Contacts

| Role | Contact | Escalation Level |
|------|---------|------------------|
| Primary On-Call | [Your Name/Team] | L1 |
| Secondary On-Call | [Backup Engineer] | L2 |
| Team Lead | [Team Lead] | L3 |
| PagerDuty | [Link] | - |
| Slack Channel | #mev-bot-alerts | - |

---

# 1. 🚀 Starting the Bot

## 1.1 Pre-Flight Checks

Before starting the bot, perform these critical checks:

### System Requirements Check

```bash
# Check system resources
free -h
# Expected: At least 4GB free memory

df -h /opt/solana-bot
# Expected: At least 10GB free disk space

# Check network connectivity
ping -c 3 8.8.8.8
# Expected: < 50ms latency, 0% packet loss

# Check Solana network status
solana cluster-version
# Expected: Should return version without errors
```

### Configuration Validation

```bash
# Navigate to bot directory
cd /opt/solana-bot

# Verify config file exists and is valid
test -f config.toml && echo "✓ Config exists" || echo "✗ Config missing"

# Check critical config values
grep -E "(rpc_url|keypair_path|min_profit_threshold|max_position_size)" config.toml

# Expected values for Phase 1:
# - rpc_url: Valid Helius/Quicknode endpoint
# - keypair_path: Points to existing file
# - min_profit_threshold: 0.001 (or higher)
# - max_position_size: 0.1 (Phase 1 limit)
```

### Keypair Validation

```bash
# Check keypair file exists
KEYPAIR_PATH=$(grep keypair_path config.toml | cut -d'"' -f2)
test -f "$KEYPAIR_PATH" && echo "✓ Keypair exists" || echo "✗ Keypair missing"

# Check keypair permissions (must be 600)
ls -la "$KEYPAIR_PATH"
# Expected: -rw------- (600 permissions)

# Get wallet address
WALLET_ADDRESS=$(solana-keygen pubkey "$KEYPAIR_PATH")
echo "Wallet Address: $WALLET_ADDRESS"

# Check wallet balance
solana balance "$WALLET_ADDRESS"
# Expected: > 0.1 SOL for Phase 1
```

### RPC Endpoint Check

```bash
# Test RPC connectivity
RPC_URL=$(grep -A5 "^\[rpc\]" config.toml | grep "^url" | cut -d'"' -f2)
echo "Testing RPC: $RPC_URL"

curl -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
  --max-time 5

# Expected: {"jsonrpc":"2.0","result":"ok","id":1}
```

### Process Check

```bash
# Ensure no existing bot processes
pgrep -af mev-bot

# If found, decide whether to kill or investigate
# To kill: sudo systemctl stop solana-arbitrage-bot
```

### Pre-Flight Checklist

- [ ] System has sufficient memory (>4GB free)
- [ ] System has sufficient disk space (>10GB free)
- [ ] Network connectivity is good
- [ ] Config file exists and is valid
- [ ] Keypair file exists with correct permissions (600)
- [ ] Wallet has sufficient balance (>0.1 SOL for Phase 1)
- [ ] RPC endpoint is reachable and healthy
- [ ] No existing bot processes running
- [ ] Correct deployment phase configured

**If all checks pass, proceed to startup.**

---

## 1.2 Startup Commands

### Method 1: Systemd Service (Recommended)

```bash
# Start the service
sudo systemctl start solana-arbitrage-bot

# Expected output: (none if successful)

# Verify service started
sudo systemctl status solana-arbitrage-bot

# Expected:
# ● solana-arbitrage-bot.service - Solana MEV Arbitrage Bot
#    Loaded: loaded
#    Active: active (running) since...
#    Main PID: [number]
```

### Method 2: Direct Execution

```bash
# Navigate to bot directory
cd /opt/solana-bot

# Start bot directly (for debugging)
sudo -u solana ./mev-bot --config config.toml

# Or with logging
sudo -u solana ./mev-bot --config config.toml 2>&1 | tee logs/bot-$(date +%Y%m%d-%H%M%S).log
```

### Method 3: Background Mode

```bash
# Start in background with nohup
cd /opt/solana-bot
nohup sudo -u solana ./mev-bot --config config.toml > logs/bot.log 2>&1 &
echo $! > bot.pid

# Save PID for later stopping
BOT_PID=$(cat bot.pid)
echo "Bot PID: $BOT_PID"
```

---

## 1.3 Verification Steps

Perform these checks within **5 minutes** of startup:

### Step 1: Process Verification

```bash
# Check process is running
pgrep -af mev-bot

# Expected: Single process with correct config path

# Check process resource usage
ps aux | grep mev-bot | grep -v grep

# Expected: 
# - USER: solana
# - CPU: < 50% initially
# - MEM: < 10%
```

### Step 2: Log Verification

```bash
# View recent logs (systemd)
sudo journalctl -u solana-arbitrage-bot -n 50 --no-pager

# Or direct logs
tail -50 /opt/solana-bot/logs/bot-*.log

# Look for these indicators:
# ✓ "Bot initialized successfully"
# ✓ "Connected to RPC"
# ✓ "Loaded pools from DEXs"
# ✓ "Starting arbitrage detection"
#
# ✗ No ERROR or PANIC messages
# ✗ No "Connection refused" errors
# ✗ No "Insufficient balance" warnings
```

### Step 3: Metrics Verification

```bash
# Check metrics endpoint is responding
curl -s http://localhost:9090/metrics | head -20

# Expected: Prometheus format metrics output

# Check key metrics exist
curl -s http://localhost:9090/metrics | grep -E "(opportunities_detected|transactions_sent)"

# Expected:
# opportunities_detected 0  (will increase over time)
# transactions_sent 0       (will increase when opportunities found)
```

### Step 4: Network Connectivity

```bash
# Check if bot can reach RPC
sudo journalctl -u solana-arbitrage-bot --since "2 minutes ago" | grep -i "rpc\|connection"

# Expected: No connection errors

# Test RPC from bot's perspective
curl -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}' \
  --max-time 5

# Expected: Returns current slot number
```

### Step 5: Detection Activity

Wait 5-10 minutes, then check:

```bash
# Check if opportunities are being detected
curl -s http://localhost:9090/metrics | grep opportunities_detected

# Expected: Number > 0 (should increase over time)

# Check recent log activity
sudo journalctl -u solana-arbitrage-bot --since "5 minutes ago" | grep -i "opportunity"

# Expected: Should see "Arbitrage opportunity detected" messages
```

### Startup Verification Checklist

- [ ] Process is running under 'solana' user
- [ ] No ERROR or PANIC in logs
- [ ] Metrics endpoint responding
- [ ] RPC connectivity confirmed
- [ ] Opportunities being detected (after 5-10 min)
- [ ] CPU usage < 50%
- [ ] Memory usage < 10%

**If all verifications pass, bot is running normally.**

---

# 2. 📊 Monitoring

## 2.1 Key Metrics to Watch

### Critical Metrics (Alert if Abnormal)

| Metric | Normal Range | Warning Threshold | Critical Threshold |
|--------|--------------|-------------------|-------------------|
| **opportunities_detected** | 10-100/hour | < 5/hour | 0/hour for 30 min |
| **transactions_sent** | 2-20/hour | < 1/hour | 0/hour for 2 hours |
| **transactions_failed** | < 20% of sent | > 30% | > 50% |
| **circuit_breaker_triggered** | 0 | 1 | > 2 in 24 hours |
| **detection_latency_ms (p95)** | < 50ms | > 100ms | > 200ms |
| **profit_per_trade_sol (avg)** | > 0.001 | < 0.001 | Negative |
| **memory_usage_mb** | 100-500 MB | > 1000 MB | > 1500 MB |
| **cpu_usage_percent** | 10-40% | > 60% | > 80% |

### How to Check Metrics

```bash
# Get all current metrics
curl -s http://localhost:9090/metrics

# Check specific metric
curl -s http://localhost:9090/metrics | grep metric_name

# Watch metric in real-time (updates every 5 seconds)
watch -n 5 'curl -s http://localhost:9090/metrics | grep opportunities_detected'

# Calculate rates (opportunities per minute)
BEFORE=$(curl -s http://localhost:9090/metrics | grep opportunities_detected | awk '{print $2}')
sleep 60
AFTER=$(curl -s http://localhost:9090/metrics | grep opportunities_detected | awk '{print $2}')
echo "Opportunities per minute: $((AFTER - BEFORE))"
```

---

## 2.2 Normal vs Abnormal Behavior

### Normal Behavior Indicators

**Logs:**
```
✓ Regular "Arbitrage opportunity detected" messages (every few minutes)
✓ Occasional "Transaction sent successfully" 
✓ "Pool data updated" messages (every 10-30 seconds)
✓ No ERROR or WARN messages
✓ Steady detection latency (< 100ms)
```

**Metrics:**
```
✓ opportunities_detected increasing steadily
✓ transactions_sent increasing (but lower than opportunities)
✓ transactions_failed < 20% of transactions_sent
✓ circuit_breaker_triggered = 0
✓ Memory usage stable (not increasing over time)
✓ CPU usage 10-40%
```

**System:**
```
✓ Process running continuously
✓ No restarts
✓ Network latency < 100ms to RPC
✓ Wallet balance decreasing slowly (gas costs) but overall profit positive
```

### Abnormal Behavior Indicators

**🔴 CRITICAL (Immediate Action Required):**

```
✗ "PANIC" or "fatal error" in logs
✗ Process crashed/not running
✗ circuit_breaker_triggered > 0
✗ transactions_failed rate > 50%
✗ No opportunities detected for > 30 minutes
✗ Memory usage > 1.5GB or growing rapidly
✗ Negative total profit for > 2 hours
✗ "Insufficient balance" errors
```

**🟡 WARNING (Investigate Soon):**

```
⚠ Few opportunities detected (< 5/hour)
⚠ High transaction failure rate (20-50%)
⚠ Detection latency > 100ms
⚠ CPU usage > 60%
⚠ Memory usage > 1GB
⚠ Repeated RPC connection errors
⚠ Win rate < 60%
```

---

## 2.3 Dashboard Links

### Grafana Dashboards

**Main Dashboard:**
```
http://grafana.example.com/d/mev-bot-main
```

**Panels to Monitor:**
- Opportunities Detected (rate)
- Transactions Sent/Failed (rate)
- Success Rate (%)
- Profit per Trade (SOL)
- Total P&L (SOL)
- Circuit Breaker Status
- Detection Latency (p50, p95, p99)
- Memory Usage
- CPU Usage

**Performance Dashboard:**
```
http://grafana.example.com/d/mev-bot-performance
```

**Panels:**
- Latency breakdown (detection, building, sending)
- Pool fetch latency
- RPC response times
- Transaction confirmation times

**Financial Dashboard:**
```
http://grafana.example.com/d/mev-bot-financial
```

**Panels:**
- Cumulative P&L
- Daily profit/loss
- Profit per trade distribution
- Gas costs
- Net profit after costs
- ROI %

### Prometheus Queries

Access Prometheus at: `http://prometheus.example.com`

**Useful Queries:**

```promql
# Opportunity detection rate (per minute)
rate(opportunities_detected[1m]) * 60

# Transaction success rate
rate(transactions_sent[5m]) / (rate(transactions_sent[5m]) + rate(transactions_failed[5m])) * 100

# Average profit per trade (last hour)
rate(profit_per_trade_sol_sum[1h]) / rate(profit_per_trade_sol_count[1h])

# Circuit breaker triggers (last 24 hours)
increase(circuit_breaker_triggered[24h])

# Memory usage trend
memory_usage_bytes / 1024 / 1024

# Detection latency p95
histogram_quantile(0.95, rate(detection_latency_ms_bucket[5m]))
```

### Logs (Journald)

```bash
# Live tail
sudo journalctl -u solana-arbitrage-bot -f

# Last 100 lines
sudo journalctl -u solana-arbitrage-bot -n 100

# Errors only
sudo journalctl -u solana-arbitrage-bot -p err

# Specific time range
sudo journalctl -u solana-arbitrage-bot --since "1 hour ago"
```

---

# 3. 🚨 Incident Response

## 3.1 Bot Stopped Responding

### Symptoms
- Process not running
- No log activity
- Metrics endpoint not responding
- No opportunities being detected

### Diagnostic Steps

```bash
# 1. Check if process exists
pgrep -af mev-bot
# If no output: process is dead

# 2. Check systemd service status
sudo systemctl status solana-arbitrage-bot

# 3. Check recent logs for crash reason
sudo journalctl -u solana-arbitrage-bot -n 200 --no-pager | grep -i "error\|panic\|fatal"

# 4. Check system resources
free -h
df -h /opt/solana-bot

# 5. Check for OOM kill
sudo journalctl -k | grep -i "killed process"

# 6. Check network connectivity
ping -c 3 8.8.8.8
curl -X POST "$RPC_URL" -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
```

### Resolution Steps

**If process crashed due to panic/error:**

```bash
# 1. Review crash logs
sudo journalctl -u solana-arbitrage-bot -n 500 | less

# 2. Check if it's a known issue (see Troubleshooting section)

# 3. Attempt restart
sudo systemctl restart solana-arbitrage-bot

# 4. Monitor for 5 minutes
watch -n 10 'sudo systemctl status solana-arbitrage-bot'

# 5. If crashes again, escalate to L2
```

**If killed by OOM:**

```bash
# 1. Check memory limits
sudo systemctl show solana-arbitrage-bot -p MemoryMax

# 2. Increase memory limit temporarily
sudo systemctl edit solana-arbitrage-bot
# Add: MemoryMax=4G

sudo systemctl daemon-reload
sudo systemctl restart solana-arbitrage-bot

# 3. Monitor memory usage
watch -n 5 'sudo systemctl show solana-arbitrage-bot -p MemoryCurrent'

# 4. If still growing, indicates memory leak - escalate
```

**If network connectivity issues:**

```bash
# 1. Verify RPC endpoint is reachable
curl -v -X POST "$RPC_URL" -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'

# 2. Check DNS resolution
nslookup $(echo "$RPC_URL" | sed 's|https\?://||' | cut -d'/' -f1)

# 3. Try alternative RPC endpoint
# Edit config.toml to use backup RPC

# 4. Restart bot
sudo systemctl restart solana-arbitrage-bot
```

### Escalation Criteria

Escalate to L2 if:
- Bot crashes more than 3 times in 1 hour
- Memory leak suspected (memory continuously growing)
- Cannot determine root cause within 15 minutes
- Network issues persist > 30 minutes

---

## 3.2 Negative Profit for > 1 Hour

### Symptoms
- Total P&L decreasing
- Most trades unprofitable
- High gas costs relative to profit
- Win rate < 50%

### Diagnostic Steps

```bash
# 1. Check current P&L metrics
curl -s http://localhost:9090/metrics | grep -E "(profit|transactions)"

# 2. Calculate recent win rate
SENT=$(curl -s http://localhost:9090/metrics | grep transactions_sent | awk '{print $2}')
FAILED=$(curl -s http://localhost:9090/metrics | grep transactions_failed | awk '{print $2}')
echo "Success rate: $(echo "scale=2; $SENT / ($SENT + $FAILED) * 100" | bc)%"

# 3. Check recent logs for failures
sudo journalctl -u solana-arbitrage-bot --since "1 hour ago" | grep -i "transaction failed\|error"

# 4. Check gas costs
sudo journalctl -u solana-arbitrage-bot --since "1 hour ago" | grep -i "gas cost"

# 5. Check if min_profit_threshold is too low
grep min_profit_threshold config.toml

# 6. Check market conditions (Solana network congestion)
solana transaction-count --url "$RPC_URL"
```

### Resolution Steps

**If gas costs too high:**

```bash
# 1. Check current compute budget
sudo journalctl -u solana-arbitrage-bot -n 100 | grep "compute units"

# 2. Temporarily increase min_profit_threshold
# This ensures we only take high-profit opportunities
sudo systemctl stop solana-arbitrage-bot

# Edit config.toml
sudo nano /opt/solana-bot/config.toml
# Change: min_profit_threshold = 0.005  # From 0.001

sudo systemctl start solana-arbitrage-bot
```

**If high failure rate:**

```bash
# 1. Check circuit breaker status
curl -s http://localhost:9090/metrics | grep circuit_breaker

# 2. If circuit breaker triggered, investigate failures
sudo journalctl -u solana-arbitrage-bot --since "2 hours ago" | grep "Transaction failed" | head -20

# 3. Common failure reasons:
#    - Slippage too high (market moved)
#    - Insufficient liquidity
#    - RPC lag
#    - Network congestion

# 4. Temporarily stop bot to prevent further losses
sudo systemctl stop solana-arbitrage-bot

# 5. Analyze root cause before restarting
# 6. Escalate to L2 for strategy review
```

**If market conditions unfavorable:**

```bash
# 1. Check Solana network status
curl https://status.solana.com/api/v2/status.json | jq

# 2. Check recent slot times (should be ~400ms)
solana block-time --url "$RPC_URL"

# 3. If network congested, consider:
#    - Temporarily stopping bot
#    - Waiting for normal conditions
#    - Increasing priority fees

# 4. Monitor for 30 minutes
# 5. If conditions don't improve, stop bot until resolved
```

### Escalation Criteria

Escalate to L2 if:
- Negative profit persists > 2 hours
- Loss exceeds 5 SOL
- Win rate < 40%
- Cannot identify root cause within 30 minutes
- Market manipulation suspected

---

## 3.3 Circuit Breaker Triggered

### Symptoms
- `circuit_breaker_triggered` metric > 0
- "Circuit breaker opened" in logs
- Bot stops sending transactions
- High recent failure rate

### Diagnostic Steps

```bash
# 1. Confirm circuit breaker status
curl -s http://localhost:9090/metrics | grep circuit_breaker_triggered

# 2. Check when it triggered
sudo journalctl -u solana-arbitrage-bot | grep -i "circuit breaker" | tail -5

# 3. Count recent failures
sudo journalctl -u solana-arbitrage-bot --since "1 hour ago" | grep -c "Transaction failed"

# 4. Review failure reasons
sudo journalctl -u solana-arbitrage-bot --since "1 hour ago" | grep "Transaction failed" -A 3 | head -50

# 5. Check circuit breaker config
grep -A 5 "^\[circuit_breaker\]" config.toml
```

### Resolution Steps

**Step 1: Investigate Root Cause**

```bash
# Analyze last 20 failures
sudo journalctl -u solana-arbitrage-bot | grep "Transaction failed" -B 2 -A 5 | tail -100 > /tmp/failures.log

# Common patterns to look for:
grep -E "(slippage|insufficient|timeout|rpc)" /tmp/failures.log

# Determine if issue is:
# A) Transient (RPC hiccup, network congestion)
# B) Systematic (config issue, strategy problem)
# C) External (RPC provider issues, Solana network issues)
```

**Step 2: Resolve Based on Root Cause**

**If transient (RPC hiccup):**

```bash
# 1. Wait for circuit breaker cooldown (usually 5-10 minutes)
# Circuit breaker will auto-reset

# 2. Monitor for auto-recovery
watch -n 10 'curl -s http://localhost:9090/metrics | grep circuit_breaker'

# 3. If auto-recovered, verify normal operation
# 4. Continue monitoring for 30 minutes
```

**If systematic (config/strategy issue):**

```bash
# 1. Stop bot immediately
sudo systemctl stop solana-arbitrage-bot

# 2. Review and adjust configuration
sudo nano /opt/solana-bot/config.toml

# Consider adjusting:
# - min_profit_threshold (increase)
# - max_position_size (decrease)
# - slippage_tolerance (increase cautiously)
# - circuit_breaker.failure_threshold (increase if too sensitive)

# 3. Test configuration
sudo -u solana /opt/solana-bot/mev-bot --config /opt/solana-bot/config.toml --dry-run

# 4. If test passes, restart bot
sudo systemctl start solana-arbitrage-bot

# 5. Monitor closely for 1 hour
```

**If external (RPC/network issues):**

```bash
# 1. Check RPC provider status
# Visit provider's status page (e.g., status.helius.dev)

# 2. Check Solana network health
curl https://status.solana.com/api/v2/status.json | jq '.status.indicator'
# Expected: "none" (all systems operational)

# 3. If RPC down, switch to backup RPC
sudo nano /opt/solana-bot/config.toml
# Update rpc_url to backup

sudo systemctl restart solana-arbitrage-bot

# 4. If Solana network degraded, consider stopping bot until resolved
```

**Step 3: Manual Circuit Breaker Reset (if needed)**

```bash
# If circuit breaker doesn't auto-reset, may need manual intervention

# 1. Stop bot
sudo systemctl stop solana-arbitrage-bot

# 2. Circuit breaker state is in-memory, so restart clears it

# 3. Ensure root cause is fixed before restarting

# 4. Start bot
sudo systemctl start solana-arbitrage-bot

# 5. Verify circuit breaker reset
curl -s http://localhost:9090/metrics | grep circuit_breaker_triggered
# Expected: 0
```

### Escalation Criteria

Escalate to L2 if:
- Circuit breaker triggers > 2 times in 24 hours
- Root cause unclear after 30 minutes investigation
- Systematic issue requiring code changes
- Loss exceeds 5 SOL before circuit breaker triggered

---

## 3.4 RPC Connection Issues

### Symptoms
- "Connection refused" errors in logs
- "RPC timeout" errors
- No opportunities detected
- High RPC request latency
- Transactions not confirming

### Diagnostic Steps

```bash
# 1. Test RPC connectivity
RPC_URL=$(grep -A5 "^\[rpc\]" config.toml | grep "^url" | cut -d'"' -f2)
curl -v -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
  --max-time 5

# 2. Measure RPC latency
time curl -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}'
# Expected: < 100ms

# 3. Check DNS resolution
RPC_HOST=$(echo "$RPC_URL" | sed 's|https\?://||' | cut -d'/' -f1)
nslookup "$RPC_HOST"

# 4. Check recent RPC errors in logs
sudo journalctl -u solana-arbitrage-bot --since "10 minutes ago" | grep -i "rpc\|connection\|timeout"

# 5. Check network connectivity
ping -c 10 "$RPC_HOST"
# Expected: < 100ms latency, 0% loss

# 6. Check RPC provider status
# Visit provider's status page (Helius, Quicknode, etc.)
```

### Resolution Steps

**If RPC endpoint down:**

```bash
# 1. Confirm endpoint is down
curl "$RPC_URL"
# If no response or error, endpoint is down

# 2. Stop bot to prevent failed transactions
sudo systemctl stop solana-arbitrage-bot

# 3. Switch to backup RPC endpoint
sudo nano /opt/solana-bot/config.toml

# Change:
# [rpc]
# url = "https://backup-rpc-endpoint.com"

# 4. Test backup RPC
curl -X POST "https://backup-rpc-endpoint.com" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'

# 5. Restart bot with new RPC
sudo systemctl start solana-arbitrage-bot

# 6. Monitor for normal operation
sudo journalctl -u solana-arbitrage-bot -f
```

**If high RPC latency:**

```bash
# 1. Measure current latency
for i in {1..10}; do
  time curl -X POST "$RPC_URL" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}'
  sleep 1
done
# If consistently > 200ms, RPC is slow

# 2. Check if it's network or RPC issue
ping -c 20 "$RPC_HOST"
# If ping is fast but RPC slow, it's RPC server

# 3. If RPC slow, switch to faster endpoint
# Use geographic region close to bot server

# 4. Consider using dedicated/paid RPC for better performance

# 5. Restart bot with new RPC
```

**If intermittent connection issues:**

```bash
# 1. Check for network instability
mtr -c 100 "$RPC_HOST"
# Look for packet loss or high latency spikes

# 2. If local network issue, investigate:
#    - Switch to different network interface
#    - Check firewall rules
#    - Check DNS server

# 3. Implement retry logic (should be in bot code)
# Verify bot has proper retry/backoff

# 4. If issue persists, escalate to network team
```

**If rate limited:**

```bash
# 1. Check for rate limit errors
sudo journalctl -u solana-arbitrage-bot | grep -i "rate limit\|429\|too many requests"

# 2. If rate limited:
#    - Reduce poll frequency (increase pool_update_interval_secs)
#    - Upgrade to higher tier RPC plan
#    - Use multiple RPC endpoints with load balancing

# 3. Edit config temporarily
sudo nano /opt/solana-bot/config.toml
# Increase: pool_update_interval_secs = 30  # From 10

sudo systemctl restart solana-arbitrage-bot
```

### Escalation Criteria

Escalate to L2 if:
- RPC issues persist > 30 minutes
- Cannot reach any RPC endpoint
- Need to provision new RPC service
- Network infrastructure issues
- Rate limits cannot be resolved

---

# 4. 🔧 Routine Maintenance

## 4.1 Log Rotation

Logs can grow large over time. Implement regular rotation to prevent disk space issues.

### Journald Logs (Systemd)

**Check current log usage:**

```bash
# Check journal disk usage
sudo journalctl --disk-usage

# Expected: < 1GB
# Warning: > 2GB
# Critical: > 5GB
```

**Configure log rotation:**

```bash
# Edit journald config
sudo nano /etc/systemd/journald.conf

# Set these values:
[Journal]
SystemMaxUse=1G          # Max total disk usage
SystemMaxFileSize=100M   # Max per-file size
SystemMaxFiles=10        # Max number of files
MaxRetentionSec=604800   # Keep logs for 7 days (1 week)

# Restart journald
sudo systemctl restart systemd-journald

# Verify settings
sudo journalctl --disk-usage
```

**Manual log cleanup:**

```bash
# Clean logs older than 7 days
sudo journalctl --vacuum-time=7d

# Or clean logs to reduce size to 500MB
sudo journalctl --vacuum-size=500M

# Or keep only last 1000 entries
sudo journalctl --vacuum-files=10
```

### Application Logs (Direct Log Files)

**If bot writes to files in logs/ directory:**

```bash
# Check log directory size
du -sh /opt/solana-bot/logs

# List large log files
du -h /opt/solana-bot/logs/* | sort -h | tail -10

# Rotate logs manually
cd /opt/solana-bot/logs
for log in *.log; do
  if [ -f "$log" ]; then
    mv "$log" "$log.$(date +%Y%m%d-%H%M%S)"
    gzip "$log.$(date +%Y%m%d-%H%M%S)"
  fi
done

# Delete logs older than 30 days
find /opt/solana-bot/logs -name "*.log.*.gz" -mtime +30 -delete

# Create logrotate config
sudo tee /etc/logrotate.d/solana-bot > /dev/null << 'EOF'
/opt/solana-bot/logs/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0644 solana solana
    postrotate
        systemctl reload solana-arbitrage-bot > /dev/null 2>&1 || true
    endscript
}
EOF

# Test logrotate config
sudo logrotate -d /etc/logrotate.d/solana-bot
```

**Schedule:** Run log cleanup weekly (Sunday 2 AM)

---

## 4.2 Database Cleanup

If bot uses local database/cache for pool data or metrics:

### Metrics Database (Prometheus)

**Check Prometheus data size:**

```bash
# If running Prometheus locally
du -sh /var/lib/prometheus/data
```

**Configure retention:**

```bash
# Edit Prometheus config
sudo nano /etc/prometheus/prometheus.yml

# Add/modify retention period
storage:
  tsdb:
    retention.time: 30d    # Keep 30 days
    retention.size: 10GB   # Or max 10GB
```

### Pool Data Cache

**If bot caches pool data locally:**

```bash
# Check cache size
du -sh /opt/solana-bot/cache

# Clear old cache (if bot supports it)
sudo systemctl stop solana-arbitrage-bot
rm -rf /opt/solana-bot/cache/*
sudo systemctl start solana-arbitrage-bot

# Bot will rebuild cache from DEXs
```

**Schedule:** Clean cache monthly (1st of month, 3 AM)

---

## 4.3 Configuration Updates

When updating bot configuration without downtime:

### Non-Critical Changes (Can Wait for Restart)

Examples: min_profit_threshold, max_position_size, log level

```bash
# 1. Backup current config
sudo cp /opt/solana-bot/config.toml /opt/solana-bot/config.toml.backup.$(date +%Y%m%d)

# 2. Edit config
sudo nano /opt/solana-bot/config.toml

# 3. Validate syntax (if validator available)
# toml-validator /opt/solana-bot/config.toml

# 4. Schedule restart during low-activity period
# Check recent activity first
curl -s http://localhost:9090/metrics | grep opportunities_detected

# 5. Restart bot
sudo systemctl restart solana-arbitrage-bot

# 6. Verify new config applied
sudo journalctl -u solana-arbitrage-bot -n 50 | grep -i "config\|loaded"

# 7. Monitor for 15 minutes to ensure stable
watch -n 10 'curl -s http://localhost:9090/metrics | grep opportunities_detected'
```

### Critical Changes (Requires Testing)

Examples: RPC endpoint, keypair path, DEX contracts

```bash
# 1. Test new configuration in dry-run mode first
sudo -u solana /opt/solana-bot/mev-bot --config /opt/solana-bot/config.toml --dry-run

# 2. If test passes, apply to production
sudo systemctl restart solana-arbitrage-bot

# 3. Monitor very closely for 30 minutes
# 4. Have rollback plan ready (restore backup config)
```

### Emergency Rollback

```bash
# 1. Stop bot
sudo systemctl stop solana-arbitrage-bot

# 2. Restore backup config
sudo cp /opt/solana-bot/config.toml.backup.YYYYMMDD /opt/solana-bot/config.toml

# 3. Start bot
sudo systemctl start solana-arbitrage-bot

# 4. Verify rollback successful
sudo journalctl -u solana-arbitrage-bot -n 50
```

### Phase Transitions

When moving from Phase 1 → Phase 2, etc.:

```bash
# 1. Verify Phase 1 success criteria met:
#    - Running for full recommended duration (1 week)
#    - Win rate > 70%
#    - Positive ROI
#    - Zero circuit breaker triggers

# 2. Update max_position_size in config
sudo nano /opt/solana-bot/config.toml
# Phase 1: max_position_size = 0.1
# Phase 2: max_position_size = 0.5
# Phase 3: max_position_size = 1.0
# Phase 4: max_position_size = 5.0

# 3. Ensure sufficient wallet balance
# Phase 2 needs ~1 SOL, Phase 3 needs ~2 SOL, Phase 4 needs ~10 SOL

# 4. Restart bot
sudo systemctl restart solana-arbitrage-bot

# 5. Monitor VERY closely for first 24 hours of new phase
```

**Schedule:** Configuration reviews bi-weekly (every other Monday)

---

# 5. ⚠️ Emergency Shutdown

## 5.1 When to Shut Down

Immediate shutdown required if:

- **Financial:**
  - Rapid unexpected losses (> 5 SOL in < 1 hour)
  - Wallet balance critical low (< 0.05 SOL)
  - Suspected exploitation or attack
  - Circuit breaker triggered repeatedly (> 3 times in 1 hour)

- **Technical:**
  - Severe memory leak (> 2GB and growing)
  - Critical bug discovered in code
  - RPC provider major outage
  - Solana network halt or instability
  - Security vulnerability identified

- **Operational:**
  - Unauthorized configuration changes detected
  - Regulatory/legal requirement
  - Planned maintenance on critical infrastructure

## 5.2 Graceful Shutdown Procedure

### Step 1: Initiate Shutdown

```bash
# Option A: Systemd (preferred)
sudo systemctl stop solana-arbitrage-bot

# Option B: Direct process (if not using systemd)
BOT_PID=$(pgrep -f mev-bot)
kill -TERM $BOT_PID  # Send SIGTERM for graceful shutdown

# Option C: Emergency kill (last resort)
pkill -9 -f mev-bot  # Force kill (may lose in-flight transactions)
```

### Step 2: Wait for Graceful Exit

```bash
# Monitor for clean shutdown (max 30 seconds)
timeout 30 bash -c 'while pgrep -f mev-bot > /dev/null; do sleep 1; echo -n "."; done'
echo ""

# If still running after 30 seconds, force kill
if pgrep -f mev-bot > /dev/null; then
  echo "Graceful shutdown timed out, forcing..."
  pkill -9 -f mev-bot
fi
```

### Step 3: Verify Shutdown

```bash
# Verify process stopped
pgrep -af mev-bot
# Expected: No output

# Check final logs
sudo journalctl -u solana-arbitrage-bot -n 50 --no-pager

# Look for:
# ✓ "Shutdown initiated"
# ✓ "Cleanup complete"
# ✓ "Bot stopped successfully"
#
# ✗ "PANIC" or "crash" messages
```

---

## 5.3 Post-Shutdown Verification

After shutdown, perform these checks:

### Financial Verification

```bash
# 1. Check final wallet balance
KEYPAIR_PATH=$(grep keypair_path /opt/solana-bot/config.toml | cut -d'"' -f2)
WALLET_ADDRESS=$(solana-keygen pubkey "$KEYPAIR_PATH")
FINAL_BALANCE=$(solana balance "$WALLET_ADDRESS" | awk '{print $1}')

echo "Final wallet balance: $FINAL_BALANCE SOL"

# 2. Check pending transactions
solana confirm -v $(solana transaction-history "$WALLET_ADDRESS" --limit 1 | tail -1 | awk '{print $1}')

# 3. Calculate final P&L from metrics
# (requires metrics snapshot before shutdown)
curl -s http://localhost:9090/metrics | grep profit > /tmp/final-metrics.txt
```

### System Verification

```bash
# 1. Check for core dumps
ls -lh /opt/solana-bot/core*
ls -lh /tmp/core*

# If found, save for analysis:
sudo cp /opt/solana-bot/core* /var/log/solana-bot/crash-$(date +%Y%m%d-%H%M%S)/

# 2. Check disk space
df -h /opt/solana-bot

# 3. Check memory was freed
free -h

# 4. Verify no zombie processes
ps aux | grep mev-bot | grep -v grep
```

### Data Integrity

```bash
# 1. Verify logs were written completely
tail -50 /opt/solana-bot/logs/*.log
# Check last log entry has timestamp (not cut off mid-line)

# 2. Backup critical data
sudo mkdir -p /var/backups/solana-bot/$(date +%Y%m%d-%H%M%S)
sudo cp -r /opt/solana-bot/logs /var/backups/solana-bot/$(date +%Y%m%d-%H%M%S)/
sudo cp /opt/solana-bot/config.toml /var/backups/solana-bot/$(date +%Y%m%d-%H%M%S)/

# 3. Export final metrics
curl -s http://localhost:9090/metrics > /var/backups/solana-bot/$(date +%Y%m%d-%H%M%S)/final-metrics.txt
```

### Post-Shutdown Checklist

- [ ] Process fully stopped (pgrep shows nothing)
- [ ] No zombie processes remaining
- [ ] Final logs written successfully
- [ ] Final wallet balance recorded
- [ ] No pending transactions
- [ ] Critical data backed up
- [ ] Metrics snapshot saved
- [ ] Disk space sufficient
- [ ] Memory freed
- [ ] Core dumps saved (if any)
- [ ] Incident documented (if emergency shutdown)

### Documentation

```bash
# Create shutdown report
tee /var/log/solana-bot/shutdown-$(date +%Y%m%d-%H%M%S).txt << EOF
=== Emergency Shutdown Report ===

Timestamp: $(date)
Initiated By: $(whoami)
Reason: [FILL IN REASON]

Pre-Shutdown State:
- Wallet Balance: [FILL IN]
- Total P&L: [FILL IN]
- Uptime: [FILL IN]
- Last Opportunity: [FILL IN]

Shutdown Details:
- Method: [systemctl stop / kill / pkill -9]
- Duration: [seconds to shutdown]
- Graceful: [yes / no]

Post-Shutdown State:
- Final Wallet Balance: [FILL IN]
- Pending Transactions: [yes / no / count]
- Data Integrity: [verified / issues found]

Next Steps:
- [FILL IN RECOVERY PLAN]

Additional Notes:
- [ANY RELEVANT INFORMATION]

===========================
EOF

# Send notification to team
echo "Emergency shutdown completed. See /var/log/solana-bot/shutdown-$(date +%Y%m%d-%H%M%S).txt" | mail -s "MEV Bot Emergency Shutdown" team@example.com
```

---

# 6. 📚 Appendix

## 6.1 Quick Reference Commands

```bash
# Status checks
sudo systemctl status solana-arbitrage-bot
pgrep -af mev-bot
curl -s http://localhost:9090/metrics | grep opportunities_detected

# Log viewing
sudo journalctl -u solana-arbitrage-bot -f
sudo journalctl -u solana-arbitrage-bot -n 100
sudo journalctl -u solana-arbitrage-bot --since "1 hour ago"

# Control commands
sudo systemctl start solana-arbitrage-bot
sudo systemctl stop solana-arbitrage-bot
sudo systemctl restart solana-arbitrage-bot

# Wallet checks
WALLET=$(solana-keygen pubkey $(grep keypair_path config.toml | cut -d'"' -f2))
solana balance $WALLET

# RPC check
RPC=$(grep -A5 "^\[rpc\]" config.toml | grep "^url" | cut -d'"' -f2)
curl -X POST "$RPC" -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'

# Performance metrics
curl -s http://localhost:9090/metrics | grep latency
curl -s http://localhost:9090/metrics | grep profit
curl -s http://localhost:9090/metrics | grep transactions
```

## 6.2 Escalation Matrix

| Severity | Response Time | Escalation Path | Example Issues |
|----------|---------------|-----------------|----------------|
| **P0 - Critical** | < 15 min | On-call → L2 → L3 → Team Lead | Bot crash, major losses, security breach |
| **P1 - High** | < 1 hour | On-call → L2 | Circuit breaker, negative P&L, high failure rate |
| **P2 - Medium** | < 4 hours | On-call | RPC issues, performance degradation |
| **P3 - Low** | < 24 hours | On-call | Config updates, routine maintenance |

## 6.3 Contact Information

| Service | URL / Contact |
|---------|---------------|
| Grafana | http://grafana.example.com |
| Prometheus | http://prometheus.example.com |
| PagerDuty | https://company.pagerduty.com |
| Slack | #mev-bot-alerts |
| Runbook | https://wiki.example.com/mev-bot-runbook |
| Code Repository | https://github.com/company/mev-bot |

## 6.4 Related Documentation

- **Deployment Guide:** `DEPLOYMENT_GUIDE.md`
- **Systemd Setup:** `SYSTEMD_SETUP_GUIDE.md`
- **Test Report:** `TEST_REPORT_ANALYSIS.md`
- **Safe Testing:** `SAFE_LOCAL_TESTING.md`
- **Optimizations:** `BELLMAN_FORD_OPTIMIZATIONS.md`

---

**END OF RUNBOOK**

**Document Version:** 1.0  
**Last Reviewed:** November 15, 2025  
**Next Review:** December 15, 2025  
**Maintained By:** SRE Team
