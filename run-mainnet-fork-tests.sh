#!/usr/bin/env bash
# Mainnet Fork Testing Quickstart Script
# 
# This script sets up and runs mainnet fork integration tests for Solana arbitrage

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}═══════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Solana Mainnet Fork Integration Testing Setup${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════${NC}\n"

# Check if solana-test-validator is installed
echo -e "${YELLOW}[1/5]${NC} Checking for solana-test-validator..."
if command -v solana-test-validator &> /dev/null; then
    VERSION=$(solana-test-validator --version | head -n 1)
    echo -e "${GREEN}✓${NC} Found: $VERSION"
else
    echo -e "${RED}✗${NC} solana-test-validator not found!"
    echo -e "  Install Solana CLI tools:"
    echo -e "  ${BLUE}sh -c \"\$(curl -sSfL https://release.solana.com/stable/install)\"${NC}"
    exit 1
fi

# Check for API keys
echo -e "\n${YELLOW}[2/5]${NC} Checking for API keys..."

if [ -z "$HELIUS_API_KEY" ]; then
    echo -e "${RED}✗${NC} HELIUS_API_KEY not set"
    echo -e "  Get your key at: ${BLUE}https://helius.dev${NC}"
    echo -e "  Then run: ${BLUE}export HELIUS_API_KEY=\"your_key_here\"${NC}"
    exit 1
else
    echo -e "${GREEN}✓${NC} HELIUS_API_KEY: ${HELIUS_API_KEY:0:8}..."
fi

if [ -z "$SOLSCAN_API_KEY" ]; then
    echo -e "${RED}✗${NC} SOLSCAN_API_KEY not set"
    echo -e "  Get your key at: ${BLUE}https://solscan.io${NC}"
    echo -e "  Then run: ${BLUE}export SOLSCAN_API_KEY=\"your_key_here\"${NC}"
    exit 1
else
    echo -e "${GREEN}✓${NC} SOLSCAN_API_KEY: ${SOLSCAN_API_KEY:0:8}..."
fi

# Kill any existing test validators
echo -e "\n${YELLOW}[3/5]${NC} Cleaning up existing test validators..."
if pkill -f solana-test-validator 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Stopped existing validators"
    sleep 2
else
    echo -e "${GREEN}✓${NC} No validators running"
fi

# Build the project
echo -e "\n${YELLOW}[4/5]${NC} Building project..."
if cargo build --tests 2>&1 | tail -10; then
    echo -e "${GREEN}✓${NC} Build successful"
else
    echo -e "${RED}✗${NC} Build failed"
    exit 1
fi

# Run tests
echo -e "\n${YELLOW}[5/5]${NC} Running tests...\n"
echo -e "${BLUE}═══════════════════════════════════════════════════════${NC}"

# Parse command line arguments
TEST_NAME=${1:-}
if [ -z "$TEST_NAME" ]; then
    echo -e "${BLUE}Running all mainnet fork tests...${NC}\n"
    cargo test --test mainnet_fork_tests -- --test-threads=1 --nocapture --ignored
else
    echo -e "${BLUE}Running specific test: $TEST_NAME${NC}\n"
    cargo test --test mainnet_fork_tests $TEST_NAME -- --nocapture --ignored
fi

# Cleanup
echo -e "\n${BLUE}═══════════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}Cleaning up...${NC}"
pkill -f solana-test-validator 2>/dev/null || true
echo -e "${GREEN}✓${NC} Done!\n"
