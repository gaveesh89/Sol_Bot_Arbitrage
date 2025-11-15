#!/bin/bash
# Start Local Validator with Mainnet Clone
# Run this in Terminal 1

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  🔒 Starting Local Test Validator                           ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Configuration
HELIUS_RPC_URL=${HELIUS_RPC_URL:-"https://api.mainnet-beta.solana.com"}
TEST_LEDGER_DIR="./test-ledger"

# Clean previous ledger
if [ -d "$TEST_LEDGER_DIR" ]; then
    echo -e "${YELLOW}🧹 Cleaning previous test ledger...${NC}"
    rm -rf "$TEST_LEDGER_DIR"
fi

# Kill any existing validator
pkill -f solana-test-validator 2>/dev/null || true
sleep 2

echo -e "${GREEN}Starting validator with mainnet clone...${NC}"
echo -e "${BLUE}   RPC URL: ${HELIUS_RPC_URL:0:50}...${NC}"
echo ""

# DEX program addresses to clone
RAYDIUM_AMM="675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
ORCA_WHIRLPOOL="whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
METEORA_POOLS="LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"

echo -e "${YELLOW}Cloning programs:${NC}"
echo -e "  • Raydium AMM: $RAYDIUM_AMM"
echo -e "  • Orca Whirlpool: $ORCA_WHIRLPOOL"
echo -e "  • Meteora DLMM: $METEORA_POOLS"
echo ""

# Ignore SIGTERM to prevent accidental termination
trap '' TERM

# Start validator
exec solana-test-validator \
    --url "$HELIUS_RPC_URL" \
    --clone $RAYDIUM_AMM \
    --clone $ORCA_WHIRLPOOL \
    --clone $METEORA_POOLS \
    --ledger "$TEST_LEDGER_DIR" \
    --reset \
    --rpc-port 8899 \
    --faucet-port 9900
