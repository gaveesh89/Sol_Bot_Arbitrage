#!/bin/bash
# Setup Test Environment and Run Bot
# Run this in Terminal 2 (after validator is ready)

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  🤖 Setting Up Test Bot                                      ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

TEST_LEDGER_DIR="./test-ledger"
TEST_WALLET="${TEST_LEDGER_DIR}/test-wallet.json"
TEST_CONFIG="${TEST_LEDGER_DIR}/test-config.toml"
LOG_DIR="./logs/bot-$(date +%Y%m%d-%H%M%S)"

mkdir -p "$LOG_DIR"
mkdir -p "$TEST_LEDGER_DIR"

# Check validator is running
echo -e "${YELLOW}🔍 Checking if validator is running...${NC}"
if ! solana cluster-version --url http://localhost:8899 &> /dev/null; then
    echo -e "${RED}❌ Local validator not found at http://localhost:8899${NC}"
    echo -e "${YELLOW}Please run './start-local-validator.sh' in another terminal first${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Validator is running${NC}"
echo ""

# Configure Solana CLI
echo -e "${YELLOW}⚙️  Configuring Solana CLI for local validator...${NC}"
solana config set --url http://localhost:8899 > /dev/null
echo -e "${GREEN}✅ Solana CLI configured${NC}"
echo ""

# Create or use existing test wallet
if [ ! -f "$TEST_WALLET" ]; then
    echo -e "${YELLOW}🔑 Creating new test wallet...${NC}"
    solana-keygen new --no-bip39-passphrase --outfile "$TEST_WALLET" --force > /dev/null 2>&1
    echo -e "${GREEN}✅ Test wallet created${NC}"
else
    echo -e "${GREEN}✅ Using existing test wallet${NC}"
fi

WALLET_ADDRESS=$(solana-keygen pubkey "$TEST_WALLET")
echo -e "${BLUE}   Address: ${WALLET_ADDRESS}${NC}"
echo ""

# Airdrop SOL
echo -e "${YELLOW}💰 Airdropping 100 SOL (fake) to test wallet...${NC}"
for attempt in {1..5}; do
    if solana airdrop 100 "$WALLET_ADDRESS" --url http://localhost:8899 > /dev/null 2>&1; then
        BALANCE=$(solana balance "$WALLET_ADDRESS" --url http://localhost:8899)
        echo -e "${GREEN}✅ Airdrop successful: ${BALANCE}${NC}"
        echo -e "${YELLOW}   (This is FAKE SOL - zero real value)${NC}"
        break
    else
        if [ $attempt -lt 5 ]; then
            echo -e "${YELLOW}   Attempt $attempt failed, retrying...${NC}"
            sleep 2
        else
            echo -e "${RED}❌ Airdrop failed after 5 attempts${NC}"
            echo -e "${YELLOW}⚠️  Continuing anyway (detection will work, transactions may fail)${NC}"
        fi
    fi
done
echo ""

# Create test configuration
echo -e "${YELLOW}⚙️  Creating test configuration...${NC}"
cat > "$TEST_CONFIG" << EOF
# SAFE LOCAL TESTING CONFIGURATION
# All transactions stay on local validator - NO MAINNET ACCESS

[rpc]
url = "http://localhost:8899"
websocket_url = "ws://localhost:8900"
commitment_level = "confirmed"

[wallet]
keypair_path = "${TEST_WALLET}"

[arbitrage]
min_profit_threshold = 0.001
max_position_size = 1.0
slippage_tolerance = 0.02

[dex]
raydium_enabled = true
orca_enabled = true
meteora_enabled = true
phoenix_enabled = false
pump_enabled = false

[circuit_breaker]
failure_threshold = 5
failure_window_secs = 60
max_consecutive_failures = 10

[monitoring]
metrics_enabled = true
metrics_port = 9090
log_level = "info"

[safety]
mode = "test"
allow_mainnet = false
dry_run_only = true
EOF

echo -e "${GREEN}✅ Configuration created: ${TEST_CONFIG}${NC}"
echo ""

# Build bot
echo -e "${YELLOW}🔨 Building bot...${NC}"
if cargo build --release --features metrics 2>&1 | tee "${LOG_DIR}/build.log" | grep -E "(Compiling|Finished)"; then
    echo -e "${GREEN}✅ Build successful${NC}"
else
    echo -e "${RED}❌ Build failed. Check ${LOG_DIR}/build.log${NC}"
    exit 1
fi
echo ""

# Display info
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  🚀 Ready to Run Bot                                         ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}Test Environment:${NC}"
echo -e "  • Validator:    http://localhost:8899 (LOCAL)"
echo -e "  • Wallet:       ${WALLET_ADDRESS:0:30}..."
echo -e "  • Config:       ${TEST_CONFIG}"
echo -e "  • Mode:         DRY RUN (simulated transactions)"
echo -e "  • Logs:         ${LOG_DIR}/"
echo ""
echo -e "${YELLOW}Choose how to run:${NC}"
echo -e "${BLUE}1. Run integration tests (recommended for first test):${NC}"
echo -e "   cargo test --test integration_tests -- --nocapture --test-threads=1"
echo ""
echo -e "${BLUE}2. Run monitoring tests:${NC}"
echo -e "   cargo test --test monitoring_tests -- --nocapture --test-threads=1"
echo ""
echo -e "${BLUE}3. Run actual bot binary (if implemented):${NC}"
echo -e "   ./target/release/mev-bot --config ${TEST_CONFIG}"
echo ""
echo -e "${BLUE}4. Run tests in loop for extended testing:${NC}"
echo -e "   while true; do cargo test --test integration_tests -- --test-threads=1; sleep 60; done"
echo ""
echo -e "${GREEN}Press Ctrl+C to stop anytime${NC}"
echo -e "${YELLOW}When done, stop validator in other terminal with Ctrl+C${NC}"
echo ""
