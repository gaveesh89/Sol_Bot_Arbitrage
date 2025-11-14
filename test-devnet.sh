#!/bin/bash

# Devnet Test Runner Script
# This script tests the bot on Solana Devnet

set -e

echo "============================================"
echo "🧪 Solana MEV Bot - Devnet Testing"
echo "============================================"
echo ""

# Check if devnet wallet exists
if [ ! -f ./devnet-wallet.json ]; then
    echo "❌ Error: devnet-wallet.json not found"
    echo "Run: solana-keygen new --outfile ./devnet-wallet.json --no-bip39-passphrase"
    exit 1
fi

# Check wallet balance
echo "📊 Checking wallet balance..."
BALANCE=$(solana balance --url devnet --keypair ./devnet-wallet.json 2>/dev/null || echo "0")
echo "   Balance: $BALANCE"

if [[ "$BALANCE" == "0"* ]] || [[ "$BALANCE" == "0 SOL" ]]; then
    echo "⚠️  Low balance detected. Requesting airdrop..."
    solana airdrop 2 --url devnet --keypair ./devnet-wallet.json
    sleep 2
    BALANCE=$(solana balance --url devnet --keypair ./devnet-wallet.json)
    echo "   New balance: $BALANCE"
fi

echo ""
echo "📋 Configuration:"
echo "   RPC: $(grep '^RPC_URL=' .env | cut -d'=' -f2)"
echo "   Wallet: $(solana-keygen pubkey ./devnet-wallet.json)"
echo "   Simulation Mode: $(grep '^BOT_SIMULATION_MODE=' .env | cut -d'=' -f2)"
echo ""

# Check if bot binary exists
if [ ! -f ./target/release/mev-bot ]; then
    echo "⚙️  Building bot..."
    cargo build --release
    echo ""
fi

echo "🚀 Starting bot on Devnet..."
echo "   (Press Ctrl+C to stop)"
echo ""
echo "============================================"
echo ""

# Run the bot
export RUST_LOG=info
./target/release/mev-bot
