#!/usr/bin/env bash
# Run integration tests with mainnet fork

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}═══════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Solana Mainnet Fork Integration Tests${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════${NC}\n"

# Check API key
if [ -z "$HELIUS_API_KEY" ]; then
    echo -e "${RED}❌ HELIUS_API_KEY not set${NC}"
    echo "   Run: export HELIUS_API_KEY=\"your_key_here\""
    echo "   Or source .env file"
    exit 1
fi

# Kill any existing test validators
echo -e "${YELLOW}🧹 Cleaning up...${NC}"
pkill -f solana-test-validator 2>/dev/null || true
sleep 1

# Run tests
TEST_NAME=${1:-}
if [ -z "$TEST_NAME" ]; then
    echo -e "${BLUE}🚀 Running all integration tests...${NC}\n"
    cargo test --test integration_tests -- --test-threads=1 --nocapture --ignored
else
    echo -e "${BLUE}🚀 Running specific test: $TEST_NAME${NC}\n"
    cargo test --test integration_tests $TEST_NAME -- --nocapture --ignored
fi

# Cleanup
echo -e "\n${YELLOW}🧹 Cleaning up...${NC}"
pkill -f solana-test-validator 2>/dev/null || true
echo -e "${GREEN}✅ Done!${NC}\n"
