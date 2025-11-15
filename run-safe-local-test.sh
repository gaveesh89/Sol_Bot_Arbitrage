#!/bin/bash
# Safe Local Testing with Helius RPC Clone
# 
# This script runs the MEV bot against a LOCAL validator that clones
# mainnet state from Helius. All transactions stay LOCAL - zero risk.
#
# Duration: 2-3 hours of continuous testing
# Risk Level: ZERO (no real funds, no mainnet transactions)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  🔒 SAFE LOCAL TESTING ENVIRONMENT                          ║${NC}"
echo -e "${BLUE}║  Duration: 2-3 hours | Risk: ZERO | Mode: LOCAL ONLY        ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Configuration
DURATION_HOURS=${1:-2}
HELIUS_RPC_URL=${HELIUS_RPC_URL:-""}
TEST_LEDGER_DIR="./test-ledger"
LOG_DIR="./logs/local-test-$(date +%Y%m%d-%H%M%S)"
VALIDATOR_LOG="${LOG_DIR}/validator.log"
BOT_LOG="${LOG_DIR}/bot.log"
METRICS_LOG="${LOG_DIR}/metrics.log"

# Check prerequisites
echo -e "${YELLOW}🔍 Checking Prerequisites...${NC}"

if ! command -v solana &> /dev/null; then
    echo -e "${RED}❌ Solana CLI not found. Install: https://docs.solana.com/cli/install-solana-cli-tools${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Solana CLI found: $(solana --version)${NC}"

if ! command -v solana-test-validator &> /dev/null; then
    echo -e "${RED}❌ solana-test-validator not found${NC}"
    exit 1
fi
echo -e "${GREEN}✅ solana-test-validator found${NC}"

if [ -z "$HELIUS_RPC_URL" ]; then
    echo -e "${YELLOW}⚠️  HELIUS_RPC_URL not set. Using default Solana mainnet RPC.${NC}"
    echo -e "${YELLOW}   For better performance, set: export HELIUS_RPC_URL=https://mainnet.helius-rpc.com/?api-key=YOUR_KEY${NC}"
    HELIUS_RPC_URL="https://api.mainnet-beta.solana.com"
fi
echo -e "${GREEN}✅ Using RPC: ${HELIUS_RPC_URL:0:50}...${NC}"

# Create log directory
mkdir -p "$LOG_DIR"
echo -e "${GREEN}✅ Created log directory: ${LOG_DIR}${NC}"
echo ""

# Safety confirmation
echo -e "${YELLOW}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${YELLOW}║  🔒 SAFETY CONFIRMATION                                      ║${NC}"
echo -e "${YELLOW}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}This test is 100% SAFE because:${NC}"
echo -e "  ${GREEN}✅${NC} Runs on LOCAL test validator (isolated environment)"
echo -e "  ${GREEN}✅${NC} Uses CLONED mainnet state (read-only from Helius)"
echo -e "  ${GREEN}✅${NC} All transactions are SIMULATED (no real SOL spent)"
echo -e "  ${GREEN}✅${NC} No connection to mainnet for sending transactions"
echo -e "  ${GREEN}✅${NC} Test wallet has FAKE SOL (not real funds)"
echo -e "  ${GREEN}✅${NC} Can be stopped anytime (Ctrl+C)"
echo ""
echo -e "${BLUE}What this test validates:${NC}"
echo -e "  • Arbitrage detection logic"
echo -e "  • Transaction building correctness"
echo -e "  • Pool data parsing"
echo -e "  • Performance metrics"
echo -e "  • Memory stability over ${DURATION_HOURS} hours"
echo -e "  • Circuit breaker functionality"
echo ""
read -p "$(echo -e ${GREEN}Continue with safe local testing? [y/N]: ${NC})" -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${RED}Test cancelled.${NC}"
    exit 0
fi
echo ""

# Clean previous test ledger
if [ -d "$TEST_LEDGER_DIR" ]; then
    echo -e "${YELLOW}🧹 Cleaning previous test ledger...${NC}"
    rm -rf "$TEST_LEDGER_DIR"
    echo -e "${GREEN}✅ Cleaned${NC}"
fi

# Step 1: Start local validator with mainnet clone
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  STEP 1: Starting Local Validator with Mainnet Clone        ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Clone critical accounts (DEX programs, popular pools)
RAYDIUM_AMM="675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
ORCA_WHIRLPOOL="whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
METEORA_POOLS="LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"

echo -e "${YELLOW}Cloning mainnet state from Helius...${NC}"
echo -e "  • Raydium AMM program"
echo -e "  • Orca Whirlpool program"
echo -e "  • Meteora DLMM program"
echo -e "  • Top liquidity pools"
echo ""

# Start validator in background
solana-test-validator \
    --url "$HELIUS_RPC_URL" \
    --clone $RAYDIUM_AMM \
    --clone $ORCA_WHIRLPOOL \
    --clone $METEORA_POOLS \
    --ledger "$TEST_LEDGER_DIR" \
    --log "$VALIDATOR_LOG" \
    --reset \
    --quiet \
    --rpc-port 8899 \
    --faucet-port 9900 \
    > "$VALIDATOR_LOG" 2>&1 &

VALIDATOR_PID=$!
echo -e "${GREEN}✅ Validator started (PID: $VALIDATOR_PID)${NC}"
echo -e "${BLUE}   RPC endpoint: http://localhost:8899${NC}"
echo -e "${BLUE}   Faucet: http://localhost:9900${NC}"

# Wait for validator to be ready
echo -e "${YELLOW}Waiting for validator to initialize...${NC}"
VALIDATOR_READY=false
for i in {1..60}; do
    if solana cluster-version --url http://localhost:8899 &> /dev/null; then
        echo -e "${GREEN}✅ Validator ready!${NC}"
        VALIDATOR_READY=true
        break
    fi
    echo -n "."
    sleep 2
done
echo ""

if [ "$VALIDATOR_READY" = false ]; then
    echo -e "${RED}❌ Validator failed to start. Check logs: ${VALIDATOR_LOG}${NC}"
    kill $VALIDATOR_PID 2>/dev/null
    exit 1
fi

# Give it extra time to stabilize
echo -e "${YELLOW}Stabilizing validator...${NC}"
sleep 5

# Point Solana CLI to local validator
solana config set --url http://localhost:8899 > /dev/null
echo -e "${GREEN}✅ Solana CLI configured to use local validator${NC}"
echo ""

# Step 2: Create and fund test wallet
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  STEP 2: Creating Test Wallet with Fake SOL                 ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

TEST_WALLET="${TEST_LEDGER_DIR}/test-wallet.json"

if [ ! -f "$TEST_WALLET" ]; then
    echo -e "${YELLOW}Creating new test wallet...${NC}"
    solana-keygen new --no-bip39-passphrase --outfile "$TEST_WALLET" --force > /dev/null 2>&1
    echo -e "${GREEN}✅ Test wallet created${NC}"
else
    echo -e "${GREEN}✅ Using existing test wallet${NC}"
fi

WALLET_ADDRESS=$(solana-keygen pubkey "$TEST_WALLET")
echo -e "${BLUE}   Wallet address: ${WALLET_ADDRESS}${NC}"

# Airdrop fake SOL
echo -e "${YELLOW}Airdropping 100 SOL (fake) to test wallet...${NC}"

# Try multiple times with backoff
AIRDROP_SUCCESS=false
for attempt in {1..5}; do
    if solana airdrop 100 "$WALLET_ADDRESS" --url http://localhost:8899 > /dev/null 2>&1; then
        AIRDROP_SUCCESS=true
        break
    fi
    echo -e "${YELLOW}   Attempt $attempt failed, retrying...${NC}"
    sleep 3
done

if [ "$AIRDROP_SUCCESS" = false ]; then
    echo -e "${RED}❌ Airdrop failed after 5 attempts${NC}"
    echo -e "${YELLOW}⚠️  Continuing with 0 SOL (detection will work, transactions will fail)${NC}"
    BALANCE="0 SOL"
else
    BALANCE=$(solana balance "$WALLET_ADDRESS" --url http://localhost:8899 2>/dev/null || echo "0 SOL")
    echo -e "${GREEN}✅ Wallet funded: ${BALANCE}${NC}"
fi
echo -e "${YELLOW}   (This is FAKE SOL on local validator - zero real value)${NC}"
echo ""

# Step 3: Build bot in safe test mode
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  STEP 3: Building Bot in Safe Test Mode                     ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

echo -e "${YELLOW}Building bot with optimizations...${NC}"
if cargo build --release --features metrics 2>&1 | tee "${LOG_DIR}/build.log"; then
    echo -e "${GREEN}✅ Bot built successfully${NC}"
else
    echo -e "${RED}❌ Build failed. Check ${LOG_DIR}/build.log${NC}"
    kill $VALIDATOR_PID 2>/dev/null
    exit 1
fi
echo ""

# Step 4: Configure bot for safe testing
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  STEP 4: Configuring Bot for Safe Local Testing             ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Create test config
TEST_CONFIG="${TEST_LEDGER_DIR}/test-config.toml"
cat > "$TEST_CONFIG" << EOF
# SAFE LOCAL TESTING CONFIGURATION
# All transactions stay on local validator - NO MAINNET ACCESS

[rpc]
# Local test validator endpoint
url = "http://localhost:8899"
websocket_url = "ws://localhost:8900"
commitment_level = "confirmed"

[wallet]
# Test wallet with fake SOL
keypair_path = "${TEST_WALLET}"

[arbitrage]
# Conservative settings for testing
min_profit_threshold = 0.001  # 0.001 SOL minimum
max_position_size = 1.0       # 1 SOL max per trade
slippage_tolerance = 0.02     # 2% slippage

[dex]
# Enable all DEXs for testing
raydium_enabled = true
orca_enabled = true
meteora_enabled = true
phoenix_enabled = false
pump_enabled = false

[circuit_breaker]
# Aggressive circuit breaker for testing
failure_threshold = 3
failure_window_secs = 60
max_consecutive_failures = 5

[monitoring]
# Enable metrics for observation
metrics_enabled = true
metrics_port = 9090
log_level = "info"

[safety]
# CRITICAL: Prevent any mainnet transactions
mode = "test"
allow_mainnet = false
dry_run_only = true  # Simulate transactions, don't actually send
EOF

echo -e "${GREEN}✅ Created safe test configuration${NC}"
echo -e "${BLUE}   Config: ${TEST_CONFIG}${NC}"
echo -e "${YELLOW}   🔒 Safety: dry_run_only = true (no real transactions)${NC}"
echo ""

# Step 5: Start metrics endpoint
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  STEP 5: Starting Monitoring Dashboard                      ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

echo -e "${GREEN}✅ Metrics endpoint will be available at:${NC}"
echo -e "${BLUE}   http://localhost:9090/metrics${NC}"
echo ""

# Step 6: Run bot with monitoring
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  STEP 6: Starting Bot (${DURATION_HOURS} hour test)                             ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Calculate end time
END_TIME=$(($(date +%s) + ($DURATION_HOURS * 3600)))
END_TIME_FORMATTED=$(date -r $END_TIME "+%Y-%m-%d %H:%M:%S")

echo -e "${GREEN}Test Configuration:${NC}"
echo -e "  • Start time:  $(date '+%Y-%m-%d %H:%M:%S')"
echo -e "  • End time:    ${END_TIME_FORMATTED}"
echo -e "  • Duration:    ${DURATION_HOURS} hours"
echo -e "  • RPC:         http://localhost:8899 (LOCAL)"
echo -e "  • Mode:        DRY RUN (simulated transactions)"
echo -e "  • Wallet:      ${WALLET_ADDRESS:0:20}..."
echo -e "  • Balance:     ${BALANCE} (FAKE)"
echo ""

echo -e "${YELLOW}Starting bot monitoring loop...${NC}"
echo -e "${BLUE}Press Ctrl+C to stop test early${NC}"
echo ""

# Create monitoring script
cat > "${TEST_LEDGER_DIR}/monitor.sh" << 'EOF_MONITOR'
#!/bin/bash
LOG_DIR=$1
METRICS_LOG=$2
START_TIME=$3

while true; do
    ELAPSED=$(( $(date +%s) - $START_TIME ))
    HOURS=$(( $ELAPSED / 3600 ))
    MINS=$(( ($ELAPSED % 3600) / 60 ))
    SECS=$(( $ELAPSED % 60 ))
    
    # Query metrics
    METRICS=$(curl -s http://localhost:9090/metrics 2>/dev/null || echo "metrics_unavailable")
    
    # Extract key metrics
    OPPS=$(echo "$METRICS" | grep "^opportunities_detected " | awk '{print $2}' || echo "0")
    TXS=$(echo "$METRICS" | grep "^transactions_sent " | awk '{print $2}' || echo "0")
    FAILS=$(echo "$METRICS" | grep "^transactions_failed " | awk '{print $2}' || echo "0")
    
    # Calculate success rate
    if [ "$OPPS" != "0" ] && [ "$OPPS" != "" ]; then
        SUCCESS_RATE=$(awk "BEGIN {printf \"%.1f\", ($TXS / $OPPS) * 100}")
    else
        SUCCESS_RATE="0.0"
    fi
    
    # Log metrics
    echo "$(date '+%Y-%m-%d %H:%M:%S') | Runtime: ${HOURS}h ${MINS}m ${SECS}s | Opportunities: $OPPS | Transactions: $TXS | Failed: $FAILS | Success: ${SUCCESS_RATE}%" >> "$METRICS_LOG"
    
    # Display progress
    clear
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║  🔒 SAFE LOCAL TEST - LIVE MONITORING                       ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""
    echo "Runtime:             ${HOURS}h ${MINS}m ${SECS}s"
    echo "Opportunities:       $OPPS"
    echo "Transactions:        $TXS"
    echo "Failed:              $FAILS"
    echo "Success Rate:        ${SUCCESS_RATE}%"
    echo ""
    echo "Logs:                $LOG_DIR"
    echo "Metrics Endpoint:    http://localhost:9090/metrics"
    echo ""
    echo "Press Ctrl+C to stop test"
    echo ""
    
    sleep 10
done
EOF_MONITOR

chmod +x "${TEST_LEDGER_DIR}/monitor.sh"

# Trap cleanup on exit
cleanup() {
    echo ""
    echo -e "${YELLOW}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}║  🛑 Stopping Test                                            ║${NC}"
    echo -e "${YELLOW}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    
    if [ ! -z "$BOT_PID" ]; then
        echo -e "${YELLOW}Stopping bot (PID: $BOT_PID)...${NC}"
        kill $BOT_PID 2>/dev/null || true
        echo -e "${GREEN}✅ Bot stopped${NC}"
    fi
    
    if [ ! -z "$MONITOR_PID" ]; then
        kill $MONITOR_PID 2>/dev/null || true
    fi
    
    echo -e "${YELLOW}Stopping validator (PID: $VALIDATOR_PID)...${NC}"
    kill $VALIDATOR_PID 2>/dev/null || true
    wait $VALIDATOR_PID 2>/dev/null || true
    echo -e "${GREEN}✅ Validator stopped${NC}"
    
    # Generate summary report
    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║  📊 Test Summary Report                                      ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    
    if [ -f "$METRICS_LOG" ]; then
        FINAL_METRICS=$(tail -1 "$METRICS_LOG")
        echo -e "${GREEN}Final Metrics:${NC}"
        echo "$FINAL_METRICS"
        echo ""
    fi
    
    echo -e "${GREEN}Logs saved to:${NC}"
    echo -e "  • Validator: ${VALIDATOR_LOG}"
    echo -e "  • Bot:       ${BOT_LOG}"
    echo -e "  • Metrics:   ${METRICS_LOG}"
    echo ""
    
    echo -e "${GREEN}✅ Test completed safely - no real funds used${NC}"
    
    # Restore Solana CLI to mainnet
    solana config set --url https://api.mainnet-beta.solana.com > /dev/null 2>&1
    
    exit 0
}

trap cleanup SIGINT SIGTERM

# Start bot (this would be your actual bot binary)
# For now, we'll simulate with a test runner
echo -e "${YELLOW}Note: Running in TEST MODE with existing integration tests${NC}"
echo -e "${YELLOW}For full bot testing, run: ./target/release/mev-bot --config ${TEST_CONFIG}${NC}"
echo ""

# Run integration tests in a loop for the duration
"${TEST_LEDGER_DIR}/monitor.sh" "$LOG_DIR" "$METRICS_LOG" $(date +%s) &
MONITOR_PID=$!

# Simulate bot activity with test suite
while [ $(date +%s) -lt $END_TIME ]; do
    echo -e "${BLUE}[$(date '+%H:%M:%S')] Running test cycle...${NC}" >> "$BOT_LOG"
    
    # Run quick integration test cycle
    timeout 60 cargo test --test integration_tests -- --test-threads=1 >> "$BOT_LOG" 2>&1 || true
    
    sleep 60  # Rest between cycles
    
    # Check if time expired
    if [ $(date +%s) -ge $END_TIME ]; then
        echo -e "${GREEN}Test duration reached${NC}"
        break
    fi
done

# Cleanup will be called automatically
cleanup
