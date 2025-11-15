#!/bin/bash
# Simple Test Loop - Terminal 2 Only

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Checking validator..."
if ! solana cluster-version --url http://localhost:8899 &> /dev/null; then
    echo "ERROR: Validator not running!"
    echo "Start validator in Terminal 1: ./start-local-validator.sh"
    exit 1
fi

echo -e "${GREEN}✅ Validator OK - Starting test loop...${NC}"
echo ""

CYCLE=0
while true; do
    CYCLE=$((CYCLE + 1))
    echo "═══ Cycle #${CYCLE} - $(date '+%H:%M:%S') ═══"
    cargo test --test integration_tests -- --ignored --test-threads=1 2>&1 | grep -E "(test result:|passed|failed)"
    echo "Sleeping 60s..."
    sleep 60
done
