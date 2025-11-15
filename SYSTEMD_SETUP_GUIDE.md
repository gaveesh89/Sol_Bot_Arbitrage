# 🔧 Systemd Service Setup Guide

Complete guide for installing and managing the Solana MEV Arbitrage Bot as a systemd service.

---

## 📋 Prerequisites

- Linux system with systemd (Ubuntu 20.04+, Debian 10+, RHEL 8+, etc.)
- Root/sudo access
- Bot binary and configuration files ready
- Network connectivity

---

## 🚀 Installation Steps

### Step 1: Create System User

Create a dedicated non-root user for security:

```bash
# Create 'solana' user with home directory
sudo useradd -r -s /bin/false -m -d /opt/solana-bot solana

# Verify user created
id solana
# Output: uid=xxx(solana) gid=xxx(solana) groups=xxx(solana)
```

**User Details:**
- `-r` - System user (UID < 1000)
- `-s /bin/false` - No shell access (security)
- `-m` - Create home directory
- `-d /opt/solana-bot` - Home directory path

### Step 2: Create Directory Structure

```bash
# Create main directory
sudo mkdir -p /opt/solana-bot
sudo mkdir -p /opt/solana-bot/logs

# Create config directory
sudo mkdir -p /etc/solana-bot

# Create systemd override directory (optional)
sudo mkdir -p /etc/systemd/system/solana-arbitrage-bot.service.d
```

### Step 3: Copy Bot Files

```bash
# Copy bot binary from deployment package
sudo cp deploy-*/mev-bot /opt/solana-bot/
sudo chmod +x /opt/solana-bot/mev-bot

# Copy configuration
sudo cp config.toml /opt/solana-bot/

# Copy keypair (if not using external path)
sudo cp your-keypair.json /opt/solana-bot/keypair.json
sudo chmod 600 /opt/solana-bot/keypair.json
```

### Step 4: Create Environment File

Create `/etc/solana-bot/env` with environment variables:

```bash
sudo tee /etc/solana-bot/env > /dev/null << 'EOF'
# Solana MEV Bot Environment Variables

# Logging
RUST_LOG=info
RUST_BACKTRACE=1

# RPC Configuration (override config.toml if needed)
# SOLANA_RPC_URL=https://api.mainnet-beta.solana.com

# Helius API Key (if using enhanced pool fetching)
# HELIUS_API_KEY=your-api-key-here

# Monitoring
METRICS_PORT=9090
PROMETHEUS_ENABLED=true

# Performance Tuning
# RUST_MIN_STACK=8388608  # 8MB stack size

# Custom settings
# MIN_PROFIT_THRESHOLD=0.001
# MAX_POSITION_SIZE=0.1
EOF

# Secure the file (contains sensitive data)
sudo chmod 600 /etc/solana-bot/env
sudo chown root:root /etc/solana-bot/env
```

### Step 5: Set Ownership and Permissions

```bash
# Set ownership
sudo chown -R solana:solana /opt/solana-bot

# Secure permissions
sudo chmod 755 /opt/solana-bot
sudo chmod 700 /opt/solana-bot/logs
sudo chmod 644 /opt/solana-bot/config.toml
sudo chmod 600 /opt/solana-bot/keypair.json  # Critical!
sudo chmod 755 /opt/solana-bot/mev-bot
```

### Step 6: Install Systemd Service

```bash
# Copy service file
sudo cp solana-arbitrage-bot.service /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload

# Enable service (start on boot)
sudo systemctl enable solana-arbitrage-bot

# Verify service file is valid
sudo systemd-analyze verify solana-arbitrage-bot.service
```

### Step 7: Start the Service

```bash
# Start the bot
sudo systemctl start solana-arbitrage-bot

# Check status
sudo systemctl status solana-arbitrage-bot

# Should show:
# ● solana-arbitrage-bot.service - Solana MEV Arbitrage Bot
#    Loaded: loaded (/etc/systemd/system/solana-arbitrage-bot.service; enabled)
#    Active: active (running) since ...
```

---

## 📊 Service Management

### Basic Commands

```bash
# Start service
sudo systemctl start solana-arbitrage-bot

# Stop service
sudo systemctl stop solana-arbitrage-bot

# Restart service
sudo systemctl restart solana-arbitrage-bot

# Reload configuration (if bot supports SIGHUP)
sudo systemctl reload solana-arbitrage-bot

# Check status
sudo systemctl status solana-arbitrage-bot

# Enable (start on boot)
sudo systemctl enable solana-arbitrage-bot

# Disable (don't start on boot)
sudo systemctl disable solana-arbitrage-bot

# View service configuration
sudo systemctl cat solana-arbitrage-bot
```

### Advanced Commands

```bash
# Show service properties
sudo systemctl show solana-arbitrage-bot

# Check if service is active
sudo systemctl is-active solana-arbitrage-bot

# Check if service is enabled
sudo systemctl is-enabled solana-arbitrage-bot

# List all loaded units
sudo systemctl list-units | grep solana

# Show failed services
sudo systemctl --failed

# Reset failed state
sudo systemctl reset-failed solana-arbitrage-bot
```

---

## 📝 Log Management

### Viewing Logs

```bash
# View all logs
sudo journalctl -u solana-arbitrage-bot

# Follow logs (live tail)
sudo journalctl -u solana-arbitrage-bot -f

# Show last 100 lines
sudo journalctl -u solana-arbitrage-bot -n 100

# Show logs from last hour
sudo journalctl -u solana-arbitrage-bot --since "1 hour ago"

# Show logs from today
sudo journalctl -u solana-arbitrage-bot --since today

# Show logs between times
sudo journalctl -u solana-arbitrage-bot --since "2025-11-15 10:00" --until "2025-11-15 12:00"

# Show only errors
sudo journalctl -u solana-arbitrage-bot -p err

# Show with timestamps
sudo journalctl -u solana-arbitrage-bot -o short-precise

# Show in JSON format
sudo journalctl -u solana-arbitrage-bot -o json-pretty
```

### Log Filtering

```bash
# Search for specific text
sudo journalctl -u solana-arbitrage-bot | grep "Arbitrage opportunity"

# Count opportunities
sudo journalctl -u solana-arbitrage-bot --since today | grep -c "opportunity detected"

# Show errors only
sudo journalctl -u solana-arbitrage-bot -p 3

# Show warnings and errors
sudo journalctl -u solana-arbitrage-bot -p 4
```

### Log Rotation

Configure journald log retention:

```bash
# Edit journald config
sudo nano /etc/systemd/journald.conf

# Set limits:
SystemMaxUse=1G          # Max total disk usage
SystemMaxFileSize=100M   # Max per-file size
SystemMaxFiles=10        # Max number of files
MaxRetentionSec=7day     # Keep logs for 7 days

# Restart journald
sudo systemctl restart systemd-journald
```

### Export Logs

```bash
# Export to file
sudo journalctl -u solana-arbitrage-bot > bot-logs.txt

# Export last 24 hours
sudo journalctl -u solana-arbitrage-bot --since "24 hours ago" > bot-logs-24h.txt

# Export as JSON
sudo journalctl -u solana-arbitrage-bot -o json > bot-logs.json
```

---

## 🔍 Monitoring & Health Checks

### System Resource Usage

```bash
# Show service cgroup resource usage
sudo systemd-cgtop

# Show memory usage for bot
sudo systemctl show solana-arbitrage-bot -p MemoryCurrent

# Show CPU usage
ps aux | grep mev-bot

# Detailed resource stats
sudo systemctl status solana-arbitrage-bot -l --no-pager
```

### Metrics Endpoint

```bash
# Check if metrics endpoint is working
curl http://localhost:9090/metrics

# Key metrics to monitor
curl -s http://localhost:9090/metrics | grep -E "(opportunities_detected|transactions_sent|transactions_failed|circuit_breaker)"

# Continuous monitoring
watch -n 5 'curl -s http://localhost:9090/metrics | grep opportunities_detected'
```

### Health Check Script

Create `/usr/local/bin/check-bot-health.sh`:

```bash
#!/bin/bash

# Check if service is running
if ! systemctl is-active --quiet solana-arbitrage-bot; then
    echo "❌ Service is not running"
    exit 1
fi

# Check if process exists
if ! pgrep -f mev-bot > /dev/null; then
    echo "❌ Process not found"
    exit 1
fi

# Check metrics endpoint
if ! curl -sf http://localhost:9090/metrics > /dev/null; then
    echo "❌ Metrics endpoint not responding"
    exit 1
fi

# Check for recent activity (opportunities detected in last 5 minutes)
recent_logs=$(journalctl -u solana-arbitrage-bot --since "5 minutes ago" | grep -c "opportunity" || echo 0)
echo "✅ Bot is healthy (${recent_logs} opportunities in last 5 min)"
exit 0
```

Make it executable:
```bash
sudo chmod +x /usr/local/bin/check-bot-health.sh

# Run health check
/usr/local/bin/check-bot-health.sh
```

### Automated Monitoring with Cron

```bash
# Add health check to cron (every 5 minutes)
sudo crontab -e

# Add this line:
*/5 * * * * /usr/local/bin/check-bot-health.sh >> /var/log/bot-health.log 2>&1
```

---

## ⚙️ Configuration Management

### Updating Configuration

```bash
# Edit config
sudo nano /opt/solana-bot/config.toml

# Restart service to apply changes
sudo systemctl restart solana-arbitrage-bot

# Or reload (if bot supports SIGHUP)
sudo systemctl reload solana-arbitrage-bot
```

### Environment Variables

```bash
# Edit environment file
sudo nano /etc/solana-bot/env

# Restart service
sudo systemctl restart solana-arbitrage-bot
```

### Override Service Settings

Create override file without editing main service:

```bash
# Create override
sudo systemctl edit solana-arbitrage-bot

# This opens editor, add overrides:
[Service]
MemoryMax=4G
Nice=-15
Environment="RUST_LOG=debug"

# Save and exit, then reload
sudo systemctl daemon-reload
sudo systemctl restart solana-arbitrage-bot

# View overrides
sudo systemctl cat solana-arbitrage-bot
```

---

## 🔒 Security Best Practices

### File Permissions

```bash
# Verify permissions are secure
sudo ls -la /opt/solana-bot/

# Should show:
# drwxr-xr-x  solana solana  .
# -rwxr-xr-x  solana solana  mev-bot
# -rw-r--r--  solana solana  config.toml
# -rw-------  solana solana  keypair.json  # CRITICAL!
# drwx------  solana solana  logs/

# Fix if needed
sudo chmod 600 /opt/solana-bot/keypair.json
sudo chown solana:solana /opt/solana-bot/keypair.json
```

### Audit Service Security

```bash
# Check service security settings
sudo systemd-analyze security solana-arbitrage-bot

# Score of 0.1-3.0 is excellent
# Score > 5.0 needs improvement

# View detailed security analysis
sudo systemd-analyze security solana-arbitrage-bot --no-pager
```

### Firewall Configuration

```bash
# If using UFW
sudo ufw allow from 127.0.0.1 to any port 9090  # Metrics (localhost only)

# If using firewalld
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="127.0.0.1" port protocol="tcp" port="9090" accept'
sudo firewall-cmd --reload
```

### SELinux Configuration (RHEL/CentOS)

```bash
# Check SELinux status
getenforce

# If enforcing, create policy
sudo semanage fcontext -a -t bin_t "/opt/solana-bot/mev-bot"
sudo restorecon -v /opt/solana-bot/mev-bot

# Allow network access
sudo setsebool -P nis_enabled 1
```

---

## 🐛 Troubleshooting

### Service Won't Start

**Check status:**
```bash
sudo systemctl status solana-arbitrage-bot -l --no-pager
```

**Common issues:**

#### 1. Binary not found
```bash
# Check file exists and is executable
ls -la /opt/solana-bot/mev-bot
# Fix:
sudo chmod +x /opt/solana-bot/mev-bot
```

#### 2. Config file not found
```bash
# Check config exists
ls -la /opt/solana-bot/config.toml
# Fix:
sudo cp config.toml /opt/solana-bot/
```

#### 3. Permission denied
```bash
# Check ownership
ls -la /opt/solana-bot/
# Fix:
sudo chown -R solana:solana /opt/solana-bot
```

#### 4. Keypair not readable
```bash
# Check keypair permissions
ls -la /opt/solana-bot/keypair.json
# Fix:
sudo chmod 600 /opt/solana-bot/keypair.json
sudo chown solana:solana /opt/solana-bot/keypair.json
```

#### 5. Port already in use
```bash
# Check what's using port 9090
sudo lsof -i :9090
# Fix: Kill other process or change port in config
```

### Service Crashes/Restarts

**Check recent crashes:**
```bash
# View recent logs
sudo journalctl -u solana-arbitrage-bot -n 200 --no-pager

# Check for crash patterns
sudo journalctl -u solana-arbitrage-bot | grep -i "panic\|error\|fatal"

# Check restart count
sudo systemctl show solana-arbitrage-bot -p NRestarts
```

**Common crash causes:**

#### 1. Out of Memory
```bash
# Check memory limit hit
sudo journalctl -u solana-arbitrage-bot | grep -i "memory"

# Fix: Increase memory limit
sudo systemctl edit solana-arbitrage-bot
# Add: MemoryMax=4G

sudo systemctl daemon-reload
sudo systemctl restart solana-arbitrage-bot
```

#### 2. Network Issues
```bash
# Check RPC connectivity
curl -X POST YOUR_RPC_URL \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'

# Check if RPC URL in config is correct
sudo grep "url" /opt/solana-bot/config.toml
```

#### 3. Insufficient Balance
```bash
# Check wallet balance
solana balance WALLET_ADDRESS

# Need at least 0.1 SOL for operations
```

### High Resource Usage

**Check resource consumption:**
```bash
# CPU usage
top -p $(pgrep mev-bot)

# Memory usage
sudo systemctl status solana-arbitrage-bot | grep Memory

# Detailed stats
sudo systemd-cgtop -d 5
```

**Limit resource usage:**
```bash
sudo systemctl edit solana-arbitrage-bot

# Add limits:
[Service]
CPUQuota=150%        # Limit to 1.5 CPU cores
MemoryMax=2G         # Hard memory limit
Nice=0               # Normal priority instead of -10
```

### Circuit Breaker Triggered

```bash
# Check for circuit breaker events
sudo journalctl -u solana-arbitrage-bot | grep -i "circuit breaker"

# Count recent failures
sudo journalctl -u solana-arbitrage-bot --since "1 hour ago" | grep -c "Transaction failed"

# If triggered repeatedly, investigate:
# 1. Check RPC health
# 2. Check network connectivity
# 3. Check wallet balance
# 4. Review recent failed transactions
```

### Performance Issues

**Check latency:**
```bash
# View metrics
curl -s http://localhost:9090/metrics | grep latency

# If latency too high:
# 1. Check system load (top/htop)
# 2. Check network latency to RPC
# 3. Consider increasing process priority (Nice=-15)
# 4. Disable unnecessary security features (if needed)
```

---

## 🔄 Updating the Bot

### Standard Update Process

```bash
# 1. Stop service
sudo systemctl stop solana-arbitrage-bot

# 2. Backup current version
sudo cp /opt/solana-bot/mev-bot /opt/solana-bot/mev-bot.backup
sudo cp /opt/solana-bot/config.toml /opt/solana-bot/config.toml.backup

# 3. Copy new binary
sudo cp new-deployment/mev-bot /opt/solana-bot/
sudo chmod +x /opt/solana-bot/mev-bot
sudo chown solana:solana /opt/solana-bot/mev-bot

# 4. Update config if needed
sudo nano /opt/solana-bot/config.toml

# 5. Test new version
sudo -u solana /opt/solana-bot/mev-bot --config /opt/solana-bot/config.toml --dry-run

# 6. Start service
sudo systemctl start solana-arbitrage-bot

# 7. Monitor logs
sudo journalctl -u solana-arbitrage-bot -f
```

### Rollback Process

If update fails:

```bash
# Stop service
sudo systemctl stop solana-arbitrage-bot

# Restore backup
sudo cp /opt/solana-bot/mev-bot.backup /opt/solana-bot/mev-bot
sudo cp /opt/solana-bot/config.toml.backup /opt/solana-bot/config.toml

# Start service
sudo systemctl start solana-arbitrage-bot

# Verify
sudo systemctl status solana-arbitrage-bot
```

---

## 📈 Performance Tuning

### CPU Pinning (Advanced)

Pin bot to specific CPU cores:

```bash
sudo systemctl edit solana-arbitrage-bot

[Service]
CPUAffinity=0 1 2 3
# Pin to cores 0-3
```

### Real-Time Scheduling (Maximum Performance)

```bash
sudo systemctl edit solana-arbitrage-bot

[Service]
CPUSchedulingPolicy=fifo
CPUSchedulingPriority=50
Nice=-20

# Also need to grant CAP_SYS_NICE
CapabilityBoundingSet=CAP_SYS_NICE
AmbientCapabilities=CAP_SYS_NICE
```

### Huge Pages (Advanced)

Enable huge pages for better memory performance:

```bash
# System-wide setting
echo 512 | sudo tee /proc/sys/vm/nr_hugepages

# Add to bot environment
sudo tee -a /etc/solana-bot/env > /dev/null << EOF
MALLOC_CONF=thp:always
EOF

sudo systemctl restart solana-arbitrage-bot
```

---

## 🎯 Production Checklist

Before going live with systemd service:

### Pre-Production
- [ ] Service user created (`solana`)
- [ ] Directory structure created (`/opt/solana-bot`, `/etc/solana-bot`)
- [ ] Files copied and permissions set correctly
- [ ] Keypair secured (600 permissions)
- [ ] Environment file configured
- [ ] Service file installed
- [ ] Service enabled for auto-start
- [ ] Logs accessible and rotating

### Testing
- [ ] Service starts successfully
- [ ] No errors in `journalctl` logs
- [ ] Metrics endpoint accessible (`curl http://localhost:9090/metrics`)
- [ ] Bot detects opportunities (check logs)
- [ ] Memory usage within limits
- [ ] CPU usage acceptable
- [ ] Auto-restart works (test with `sudo systemctl restart`)
- [ ] Survives system reboot

### Monitoring
- [ ] Prometheus collecting metrics
- [ ] Grafana dashboards configured
- [ ] Alert rules set up
- [ ] Health check script running
- [ ] Log retention configured
- [ ] Disk space monitored

### Security
- [ ] SELinux/AppArmor policies applied (if applicable)
- [ ] Firewall rules configured
- [ ] Only localhost can access metrics
- [ ] Keypair not world-readable
- [ ] Service runs as non-root
- [ ] Security score < 3.0 (`systemd-analyze security`)

### Documentation
- [ ] Update contact info for alerts
- [ ] Document custom configuration
- [ ] Create runbook for common issues
- [ ] Schedule regular maintenance windows

---

## 📞 Support Commands Quick Reference

```bash
# Status & Logs
sudo systemctl status solana-arbitrage-bot
sudo journalctl -u solana-arbitrage-bot -f
sudo journalctl -u solana-arbitrage-bot -n 100

# Start/Stop/Restart
sudo systemctl start solana-arbitrage-bot
sudo systemctl stop solana-arbitrage-bot
sudo systemctl restart solana-arbitrage-bot

# Enable/Disable Auto-start
sudo systemctl enable solana-arbitrage-bot
sudo systemctl disable solana-arbitrage-bot

# Configuration
sudo nano /opt/solana-bot/config.toml
sudo nano /etc/solana-bot/env
sudo systemctl edit solana-arbitrage-bot

# Monitoring
curl http://localhost:9090/metrics
sudo systemctl show solana-arbitrage-bot -p MemoryCurrent
sudo systemctl show solana-arbitrage-bot -p CPUUsageNSec

# Troubleshooting
sudo systemctl status solana-arbitrage-bot -l --no-pager
sudo journalctl -u solana-arbitrage-bot | grep -i error
sudo systemd-analyze security solana-arbitrage-bot
/usr/local/bin/check-bot-health.sh

# Updates
sudo systemctl stop solana-arbitrage-bot
sudo cp new-mev-bot /opt/solana-bot/mev-bot
sudo systemctl start solana-arbitrage-bot
```

---

## 🎓 Additional Resources

- **Systemd Documentation:** `man systemd.service`
- **Journalctl Manual:** `man journalctl`
- **Systemd Security:** `man systemd.exec`
- **Resource Control:** `man systemd.resource-control`

---

**Document Version:** 1.0  
**Last Updated:** November 15, 2025  
**Service File:** `solana-arbitrage-bot.service`
