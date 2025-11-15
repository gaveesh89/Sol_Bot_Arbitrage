#!/bin/bash
# Run Local Validator in Background (Persistent)

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  🔒 Starting Persistent Local Test Validator                ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

HELIUS_RPC_URL=${HELIUS_RPC_URL:-"https://api.mainnet-beta.solana.com"}
TEST_LEDGER_DIR="./test-ledger"
LOG_DIR="./logs"
VALIDATOR_LOG="${LOG_DIR}/validator-$(date +%Y%m%d-%H%M%S).log"

mkdir -p "$LOG_DIR"

# Kill any existing validator
echo -e "${YELLOW}Killing any existing validators...${NC}"
pkill -f solana-test-validator 2>/dev/null || true
sleep 3

# Clean previous ledger
if [ -d "$TEST_LEDGER_DIR" ]; then
    echo -e "${YELLOW}Cleaning previous test ledger...${NC}"
    rm -rf "$TEST_LEDGER_DIR"
fi

# DEX program addresses
RAYDIUM_AMM="675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
ORCA_WHIRLPOOL="whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
METEORA_POOLS="LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"

echo -e "${GREEN}Starting validator in background...${NC}"
echo -e "${BLUE}   Cloning: Raydium, Orca, Meteora${NC}"
echo -e "${BLUE}   Log file: ${VALIDATOR_LOG}${NC}"
echo ""

# Start in background with nohup
nohup solana-test-validator \
    --url "$HELIUS_RPC_URL" \
    --clone $RAYDIUM_AMM \
    --clone $ORCA_WHIRLPOOL \
    --clone $METEORA_POOLS \
    --ledger "$TEST_LEDGER_DIR" \
    --reset \
    --rpc-port 8899 \
    --faucet-port 9900 \
    > "$VALIDATOR_LOG" 2>&1 &

VALIDATOR_PID=$!
echo $VALIDATOR_PID > "${TEST_LEDGER_DIR}/validator.pid"

echo -e "${GREEN}✅ Validator started in background (PID: $VALIDATOR_PID)${NC}"
echo -e "${BLUE}   RPC endpoint: http://localhost:8899${NC}"
echo -e "${BLUE}   PID saved to: ${TEST_LEDGER_DIR}/validator.pid${NC}"
echo ""

# Wait for validator to be ready
echo -e "${YELLOW}Waiting for validator to initialize...${NC}"
for i in {1..60}; do
    if solana cluster-version --url http://localhost:8899 &> /dev/null; then
        echo -e "${GREEN}✅ Validator is ready!${NC}"
        echo ""
        echo -e "${GREEN}Validator is running in background${NC}"
        echo -e "${BLUE}To view logs: tail -f ${VALIDATOR_LOG}${NC}"
        echo -e "${BLUE}To stop: ./stop-validator.sh${NC}"
        echo ""
        exit 0
    fi
    echo -n "."
    sleep 2
done

echo ""
echo -e "${RED}❌ Validator failed to start. Check logs: ${VALIDATOR_LOG}${NC}"
exit 1
