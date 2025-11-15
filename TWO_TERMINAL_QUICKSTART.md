# 🚀 Quick Start: 2-Terminal Safe Testing

## Simple Setup (Zero Risk)

### Terminal 1: Validator
```bash
./start-local-validator.sh
```
✅ Leave running (Ctrl+C to stop)

### Terminal 2: Bot
```bash
# Setup
./setup-test-bot.sh

# Run tests
cargo test --test integration_tests -- --nocapture --test-threads=1
```

## For 2-3 Hour Test

**Terminal 2 - Continuous Testing:**
```bash
./setup-test-bot.sh

# Run in loop
while true; do 
    echo "=== $(date) ===" 
    cargo test --test integration_tests -- --test-threads=1
    sleep 60
done
```

Stop anytime: Ctrl+C → Ctrl+C (both terminals)

---

## Quick Commands

| Task | Command |
|------|---------|
| Start validator | `./start-local-validator.sh` (Terminal 1) |
| Setup bot | `./setup-test-bot.sh` (Terminal 2) |
| Run tests | `cargo test --test integration_tests -- --nocapture` |
| Watch metrics | `watch curl -s http://localhost:9090/metrics` |
| Stop everything | Ctrl+C in both terminals |
| Clean up | `pkill solana-test-validator; rm -rf test-ledger` |

## Safety

✅ Local validator only (localhost:8899)  
✅ Fake SOL (zero value)  
✅ No mainnet connection  
✅ Can stop anytime  

**Zero risk. Two terminals. That's it.**
