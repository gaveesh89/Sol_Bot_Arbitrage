#!/bin/bash
# Aggressive/Dynamic Testing Startup Script
# 
# This script demonstrates the "best of both worlds" testing strategy:
# - Real-time pool data from Helius API (accurate arbitrage detection)
# - Safe execution on local fork (no real funds at risk)

set -e

echo "🚀 Starting Aggressive/Dynamic Testing Environment"
echo "=================================================="
echo ""

# Check prerequisites
if [ -z "$HELIUS_API_KEY" ]; then
    echo "❌ Error: HELIUS_API_KEY environment variable not set"
    echo "   Please set it with: export HELIUS_API_KEY='your-key-here'"
    exit 1
fi

# Configuration
HELIUS_RPC_URL="https://mainnet.helius-rpc.com/?api-key=${HELIUS_API_KEY}"
LOCAL_RPC_URL="http://127.0.0.1:8899"
LOCAL_WS_URL="ws://127.0.0.1:8900"

echo "📋 Configuration:"
echo "   External API: ${HELIUS_RPC_URL:0:50}..."
echo "   Local RPC:    $LOCAL_RPC_URL"
echo "   Simulation:   ENABLED (safe testing mode)"
echo ""

# Kill existing validator
echo "🧹 Cleaning up existing validator..."
pkill -f solana-test-validator 2>/dev/null || true
sleep 2

# Remove old ledger
if [ -d "test-ledger" ]; then
    echo "   Removing old test-ledger..."
    rm -rf test-ledger
fi

echo ""
echo "🔧 Starting local validator with mainnet fork..."
echo "   This will:"
echo "   1. Fork mainnet state from Helius"
echo "   2. Clone DEX programs (Raydium, Orca, Meteora)"
echo "   3. Clone token mints (SOL, USDC)"
echo "   4. Start local RPC on port 8899"
echo ""

# Start validator in background
solana-test-validator \
    --url "$HELIUS_RPC_URL" \
    --reset \
    --rpc-port 8899 \
    --faucet-port 9900 \
    --quiet \
    --clone-upgradeable-program 675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8 \
    --clone-upgradeable-program whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc \
    --clone-upgradeable-program LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo \
    --clone EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \
    --clone So11111111111111111111111111111111111111112 \
    > test-ledger/validator.log 2>&1 &

VALIDATOR_PID=$!
echo "   Started validator (PID: $VALIDATOR_PID)"
echo "   Log: test-ledger/validator.log"
echo ""

# Wait for validator to be ready
echo "⏳ Waiting for validator to be ready..."
MAX_ATTEMPTS=60
ATTEMPT=0

while [ $ATTEMPT -lt $MAX_ATTEMPTS ]; do
    ATTEMPT=$((ATTEMPT + 1))
    
    # Check if validator is responding
    if curl -s -X POST $LOCAL_RPC_URL \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
        | grep -q '"result":"ok"'; then
        echo "✅ Validator ready after $ATTEMPT seconds"
        break
    fi
    
    if [ $ATTEMPT -eq $MAX_ATTEMPTS ]; then
        echo "❌ Validator failed to start within 60 seconds"
        echo "   Check logs: tail -f test-ledger/validator.log"
        kill $VALIDATOR_PID 2>/dev/null || true
        exit 1
    fi
    
    sleep 1
done

echo ""
echo "🎯 Testing configuration..."

# Test local RPC
echo -n "   Local RPC: "
if curl -s -X POST $LOCAL_RPC_URL \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"getVersion"}' \
    | grep -q "solana-core"; then
    echo "✅ OK"
else
    echo "❌ FAILED"
fi

# Test external API
echo -n "   External API: "
if curl -s -X POST "$HELIUS_RPC_URL" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
    | grep -q '"result":"ok"'; then
    echo "✅ OK"
else
    echo "⚠️  WARNING (will fall back to RPC)"
fi

echo ""
echo "🚀 Starting MEV bot with dynamic testing configuration..."
echo ""
echo "=================================================="
echo "Bot will:"
echo "  ✅ Fetch real-time pool data from Helius API"
echo "  ✅ Execute transactions on local fork (safe)"
echo "  ✅ Run in simulation mode (no real funds)"
echo "=================================================="
echo ""

# Export environment variables for bot
export RPC_URL="$LOCAL_RPC_URL"
export WS_URL="$LOCAL_WS_URL"
export EXTERNAL_DATA_API_URL="$HELIUS_RPC_URL"
export BOT_SIMULATION_MODE=true

# Run bot
cargo run --release

# Cleanup on exit
echo ""
echo "🧹 Cleaning up..."
kill $VALIDATOR_PID 2>/dev/null || true
echo "✅ Done"
