#!/bin/bash
# Stop the background validator

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${YELLOW}Stopping validator...${NC}"

if [ -f "./test-ledger/validator.pid" ]; then
    PID=$(cat ./test-ledger/validator.pid)
    if kill -0 $PID 2>/dev/null; then
        kill $PID
        echo -e "${GREEN}✅ Validator stopped (PID: $PID)${NC}"
        rm ./test-ledger/validator.pid
    else
        echo -e "${YELLOW}Validator process not found${NC}"
        rm ./test-ledger/validator.pid
    fi
else
    # Fallback: kill all
    pkill -f solana-test-validator
    echo -e "${GREEN}✅ Killed all validator processes${NC}"
fi
