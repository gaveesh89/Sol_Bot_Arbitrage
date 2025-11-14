#!/bin/bash

# Mainnet Fork Testing - Quick Start Script
# This script automates the setup of a local Mainnet fork for testing

set -e

echo "============================================"
echo "🔧 Solana MEV Bot - Mainnet Fork Setup"
echo "============================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
FORK_LEDGER="/tmp/mainnet-fork-ledger"
RPC_PORT="8899"
WS_PORT="8900"
KEYPAIR_PATH="${1:-./devnet-wallet.json}"
AIRDROP_AMOUNT="${2:-100}"

# Check if RPC URL is provided
if [ -z "$MAINNET_RPC_URL" ]; then
    echo -e "${RED}❌ Error: MAINNET_RPC_URL environment variable not set${NC}"
    echo ""
    echo "Please set your high-performance RPC URL:"
    echo ""
    echo -e "${YELLOW}Example (Helius):${NC}"
    echo "  export MAINNET_RPC_URL='https://mainnet.helius-rpc.com/?api-key=YOUR_KEY'"
    echo ""
    echo -e "${YELLOW}Example (QuickNode):${NC}"
    echo "  export MAINNET_RPC_URL='https://your-endpoint.quiknode.pro/YOUR_KEY/'"
    echo ""
    echo "Then run this script again:"
    echo "  ./start-mainnet-fork.sh"
    exit 1
fi

# Check if keypair exists
if [ ! -f "$KEYPAIR_PATH" ]; then
    echo -e "${RED}❌ Error: Keypair not found at $KEYPAIR_PATH${NC}"
    echo ""
    echo "Usage: ./start-mainnet-fork.sh [KEYPAIR_PATH] [AIRDROP_AMOUNT]"
    echo ""
    echo "Examples:"
    echo "  ./start-mainnet-fork.sh                          # Use ./devnet-wallet.json, airdrop 100 SOL"
    echo "  ./start-mainnet-fork.sh ./my-wallet.json         # Use custom keypair, airdrop 100 SOL"
    echo "  ./start-mainnet-fork.sh ./my-wallet.json 50      # Use custom keypair, airdrop 50 SOL"
    exit 1
fi

# Check if solana-test-validator is installed
if ! command -v solana-test-validator &> /dev/null; then
    echo -e "${RED}❌ Error: solana-test-validator not found${NC}"
    echo ""
    echo "Please install Solana CLI tools:"
    echo "  sh -c \"\$(curl -sSfL https://release.solana.com/stable/install)\""
    exit 1
fi

# Check if port is already in use
if lsof -Pi :$RPC_PORT -sTCP:LISTEN -t >/dev/null 2>&1 ; then
    echo -e "${YELLOW}⚠️  Port $RPC_PORT is already in use${NC}"
    echo ""
    read -p "Kill existing process? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        PID=$(lsof -Pi :$RPC_PORT -sTCP:LISTEN -t)
        kill -9 $PID 2>/dev/null || true
        echo -e "${GREEN}✅ Killed process $PID${NC}"
        sleep 2
    else
        echo -e "${RED}Aborting...${NC}"
        exit 1
    fi
fi

echo -e "${BLUE}📋 Configuration:${NC}"
echo "   RPC URL: $MAINNET_RPC_URL"
echo "   Fork Ledger: $FORK_LEDGER"
echo "   RPC Port: $RPC_PORT"
echo "   WebSocket Port: $WS_PORT"
echo "   Keypair: $KEYPAIR_PATH"
echo "   Airdrop Amount: $AIRDROP_AMOUNT SOL"
echo ""

# Clean up old fork data
if [ -d "$FORK_LEDGER" ]; then
    echo -e "${YELLOW}🧹 Cleaning up old fork data...${NC}"
    rm -rf "$FORK_LEDGER"
fi

# Start the validator in the background
echo -e "${BLUE}🚀 Starting local Mainnet fork...${NC}"
echo -e "${BLUE}📦 Cloning essential Mainnet accounts...${NC}"
echo ""

# Clone multiple pools for arbitrage testing
solana-test-validator \
  --url "$MAINNET_RPC_URL" \
  --ledger "$FORK_LEDGER" \
  --reset \
  --rpc-port $RPC_PORT \
  --clone 58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2 \
  --clone 7qbRF6YsyGuLUVs6Y1q64bdVrfe4ZcUUz1JRdoVNUJnm \
  --clone HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ \
  --clone So11111111111111111111111111111111111111112 \
  --clone EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \
  --clone 675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8 \
  --clone whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc \
  --clone 9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP \
  > /tmp/fork-validator.log 2>&1 &

VALIDATOR_PID=$!

echo -e "${GREEN}✅ Validator started (PID: $VALIDATOR_PID)${NC}"
echo "   Log file: /tmp/fork-validator.log"
echo ""

# Wait for validator to be ready
echo -e "${BLUE}⏳ Waiting for validator to sync...${NC}"
for i in {1..30}; do
    if solana cluster-version --url http://127.0.0.1:$RPC_PORT &>/dev/null; then
        echo -e "${GREEN}✅ Validator is ready!${NC}"
        break
    fi
    echo -n "."
    sleep 2
    if [ $i -eq 30 ]; then
        echo ""
        echo -e "${RED}❌ Validator failed to start within 60 seconds${NC}"
        echo "Check the log: tail -f /tmp/fork-validator.log"
        kill $VALIDATOR_PID 2>/dev/null || true
        exit 1
    fi
done
echo ""

# Airdrop SOL to the test wallet
echo -e "${BLUE}💰 Funding test wallet...${NC}"
solana airdrop $AIRDROP_AMOUNT \
  --url http://127.0.0.1:$RPC_PORT \
  --keypair "$KEYPAIR_PATH"

# Verify balance
BALANCE=$(solana balance --url http://127.0.0.1:$RPC_PORT --keypair "$KEYPAIR_PATH" | awk '{print $1}')
echo -e "${GREEN}✅ Wallet funded: $BALANCE SOL${NC}"
echo ""

# Get wallet address
WALLET_ADDRESS=$(solana address --keypair "$KEYPAIR_PATH")

echo ""
echo "============================================"
echo -e "${GREEN}🎉 Mainnet Fork Ready!${NC}"
echo "============================================"
echo ""
echo -e "${BLUE}📊 Connection Details:${NC}"
echo "   RPC URL: http://127.0.0.1:$RPC_PORT"
echo "   WebSocket URL: ws://127.0.0.1:$WS_PORT"
echo "   Wallet: $WALLET_ADDRESS"
echo "   Balance: $BALANCE SOL"
echo ""
echo -e "${BLUE}🤖 To run the bot:${NC}"
echo ""
echo "   Terminal 1 (keep this open):"
echo "     # Fork is already running"
echo ""
echo "   Terminal 2 (run the bot):"
echo -e "     ${GREEN}LOCAL_FORK_URL=\"http://127.0.0.1:$RPC_PORT\" BOT_SIMULATION_MODE=false cargo run --release${NC}"
echo ""
echo -e "${BLUE}📝 Useful Commands:${NC}"
echo ""
echo "   Check validator status:"
echo "     solana cluster-version --url http://127.0.0.1:$RPC_PORT"
echo ""
echo "   Check wallet balance:"
echo "     solana balance --url http://127.0.0.1:$RPC_PORT --keypair $KEYPAIR_PATH"
echo ""
echo "   Monitor validator logs:"
echo "     tail -f /tmp/fork-validator.log"
echo ""
echo "   Monitor bot logs:"
echo "     tail -f /tmp/fork-validator.log"
echo ""
echo "   Stop the validator:"
echo "     kill $VALIDATOR_PID"
echo ""
echo -e "${YELLOW}⚠️  Important:${NC}"
echo "   - Keep this terminal open (validator is running)"
echo "   - Press Ctrl+C to stop the validator"
echo "   - Run the bot in a separate terminal"
echo ""
echo "============================================"
echo ""

# Keep the script running and monitor the validator
trap "echo ''; echo -e '${YELLOW}🛑 Stopping validator...${NC}'; kill $VALIDATOR_PID 2>/dev/null; echo -e '${GREEN}✅ Validator stopped${NC}'; exit 0" INT TERM

echo -e "${GREEN}Validator is running. Press Ctrl+C to stop.${NC}"
echo ""

# Follow the validator log
tail -f /tmp/fork-validator.log
