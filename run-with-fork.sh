#!/bin/bash

# Run the MEV bot against the local Mainnet fork
# This script sets the correct environment variables and starts the bot in LIVE mode

set -e

echo "============================================"
echo "🤖 Solana MEV Bot - Fork Testing Mode"
echo "============================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
FORK_RPC_URL="${1:-http://127.0.0.1:8899}"
KEYPAIR_PATH="${BOT_KEYPAIR_PATH:-./devnet-wallet.json}"

# Check if fork is running
if ! curl -s "$FORK_RPC_URL" -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' > /dev/null 2>&1; then
    echo -e "${RED}❌ Error: Local fork is not running at $FORK_RPC_URL${NC}"
    echo ""
    echo "Please start the fork first:"
    echo -e "  ${GREEN}export MAINNET_RPC_URL='https://your-rpc-url.com'${NC}"
    echo -e "  ${GREEN}./start-mainnet-fork.sh${NC}"
    echo ""
    exit 1
fi

# Check if keypair exists
if [ ! -f "$KEYPAIR_PATH" ]; then
    echo -e "${RED}❌ Error: Keypair not found at $KEYPAIR_PATH${NC}"
    echo ""
    echo "Set the BOT_KEYPAIR_PATH environment variable:"
    echo "  export BOT_KEYPAIR_PATH='./your-wallet.json'"
    echo ""
    exit 1
fi

# Verify wallet has funds
WALLET_ADDRESS=$(solana address --keypair "$KEYPAIR_PATH" 2>/dev/null || echo "unknown")
BALANCE=$(solana balance --url "$FORK_RPC_URL" --keypair "$KEYPAIR_PATH" 2>/dev/null | awk '{print $1}' || echo "0")

if [ "$BALANCE" == "0" ] || [ -z "$BALANCE" ]; then
    echo -e "${RED}❌ Error: Wallet has no funds${NC}"
    echo ""
    echo "Airdrop SOL to your wallet:"
    echo "  solana airdrop 100 --url $FORK_RPC_URL --keypair $KEYPAIR_PATH"
    echo ""
    exit 1
fi

echo -e "${BLUE}📋 Configuration:${NC}"
echo "   Fork RPC: $FORK_RPC_URL"
echo "   Wallet: $WALLET_ADDRESS"
echo "   Balance: $BALANCE SOL"
echo "   Keypair: $KEYPAIR_PATH"
echo "   Execution Mode: LIVE 🔥"
echo ""

echo -e "${YELLOW}⚠️  Warning: Running in LIVE mode against local fork${NC}"
echo "   - Transactions will be executed (on fork only)"
echo "   - No real funds at risk (fork uses test SOL)"
echo "   - Monitor logs for profit validation"
echo ""

read -p "Continue? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${RED}Aborting...${NC}"
    exit 1
fi

echo ""
echo -e "${BLUE}🚀 Starting bot...${NC}"
echo "   Press Ctrl+C to stop"
echo ""
echo "============================================"
echo ""

# Build if needed
if [ ! -f "./target/release/mev-bot" ]; then
    echo -e "${BLUE}🔨 Building bot...${NC}"
    cargo build --release
    echo ""
fi

# Run the bot with fork configuration
# Use LOCAL_FORK_URL (highest priority in config.rs)
LOCAL_FORK_URL="$FORK_RPC_URL" \
BOT_SIMULATION_MODE=false \
cargo run --release

echo ""
echo -e "${GREEN}✅ Bot stopped${NC}"
