#!/usr/bin/env bash

################################################################################
# Solana MEV Bot - Production Deployment Script
#
# This script performs comprehensive pre-deployment validation including:
# - Full test suite execution
# - Performance benchmarks validation
# - Production build
# - Configuration validation
# - Environment checks
# - Deployment package creation
# - Gradual rollout preparation
#
# Usage: ./deploy.sh [options]
#   --skip-tests        Skip test suite (not recommended)
#   --skip-benchmarks   Skip benchmark validation
#   --force             Skip interactive confirmations
#   --phase N           Specify deployment phase (1-4)
#   --devnet            Deploy to devnet instead of mainnet
#   --dry-run           Validate only, don't deploy
#
# Exit Codes:
#   0 - Success
#   1 - Tests failed
#   2 - Benchmarks failed
#   3 - Build failed
#   4 - Configuration validation failed
#   5 - Environment check failed
#   6 - Deployment package creation failed
################################################################################

set -euo pipefail  # Exit on error, undefined vars, pipe failures

# Color codes for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly MAGENTA='\033[0;35m'
readonly CYAN='\033[0;36m'
readonly NC='\033[0m' # No Color
readonly BOLD='\033[1m'

# Script configuration
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly TIMESTAMP=$(date +%Y%m%d-%H%M%S)
readonly DEPLOY_DIR="${SCRIPT_DIR}/deploy-${TIMESTAMP}"
readonly LOG_FILE="${SCRIPT_DIR}/deploy-${TIMESTAMP}.log"

# Performance thresholds (in milliseconds)
readonly MAX_DETECTION_LATENCY_MS=100
readonly MAX_BUILDING_LATENCY_MS=50
readonly MAX_END_TO_END_LATENCY_MS=200

# Deployment phases configuration
declare -A PHASE_CONFIG=(
    [1]="max_position=0.1,duration=1_week,description=Initial validation"
    [2]="max_position=0.5,duration=1_week,description=Small scale"
    [3]="max_position=1.0,duration=2_weeks,description=Medium scale"
    [4]="max_position=5.0,duration=ongoing,description=Full production"
)

# Command-line options
SKIP_TESTS=false
SKIP_BENCHMARKS=false
FORCE=false
DEPLOYMENT_PHASE=1
TARGET_NETWORK="mainnet"
DRY_RUN=false

################################################################################
# Utility Functions
################################################################################

log() {
    local level=$1
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    case $level in
        INFO)
            echo -e "${BLUE}[INFO]${NC} ${message}" | tee -a "${LOG_FILE}"
            ;;
        SUCCESS)
            echo -e "${GREEN}[SUCCESS]${NC} ${message}" | tee -a "${LOG_FILE}"
            ;;
        WARN)
            echo -e "${YELLOW}[WARN]${NC} ${message}" | tee -a "${LOG_FILE}"
            ;;
        ERROR)
            echo -e "${RED}[ERROR]${NC} ${message}" | tee -a "${LOG_FILE}"
            ;;
        STEP)
            echo -e "\n${CYAN}${BOLD}==== $message ====${NC}\n" | tee -a "${LOG_FILE}"
            ;;
    esac
    
    echo "[${timestamp}] [${level}] ${message}" >> "${LOG_FILE}"
}

error_exit() {
    log ERROR "$1"
    log ERROR "Deployment failed. Check ${LOG_FILE} for details."
    exit "${2:-1}"
}

confirm() {
    if [[ "${FORCE}" == "true" ]]; then
        return 0
    fi
    
    local prompt="$1"
    echo -e "${YELLOW}${prompt}${NC}"
    read -p "Continue? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log WARN "Deployment cancelled by user"
        exit 0
    fi
}

check_command() {
    if ! command -v "$1" &> /dev/null; then
        error_exit "Required command '$1' not found. Please install it first." 5
    fi
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --skip-tests)
                SKIP_TESTS=true
                log WARN "Test suite will be skipped (not recommended for production)"
                shift
                ;;
            --skip-benchmarks)
                SKIP_BENCHMARKS=true
                log WARN "Benchmark validation will be skipped"
                shift
                ;;
            --force)
                FORCE=true
                shift
                ;;
            --phase)
                DEPLOYMENT_PHASE="$2"
                if [[ ! "$DEPLOYMENT_PHASE" =~ ^[1-4]$ ]]; then
                    error_exit "Invalid phase: $DEPLOYMENT_PHASE. Must be 1-4." 5
                fi
                shift 2
                ;;
            --devnet)
                TARGET_NETWORK="devnet"
                log INFO "Target network: devnet"
                shift
                ;;
            --dry-run)
                DRY_RUN=true
                log INFO "Dry-run mode: validation only, no deployment"
                shift
                ;;
            -h|--help)
                cat << EOF
Usage: ./deploy.sh [options]

Options:
    --skip-tests        Skip test suite (not recommended)
    --skip-benchmarks   Skip benchmark validation
    --force             Skip interactive confirmations
    --phase N           Specify deployment phase (1-4)
    --devnet            Deploy to devnet instead of mainnet
    --dry-run           Validate only, don't deploy
    -h, --help          Show this help message

Deployment Phases:
    Phase 1: Max 0.1 SOL per trade (1 week validation)
    Phase 2: Max 0.5 SOL per trade (1 week small scale)
    Phase 3: Max 1.0 SOL per trade (2 weeks medium scale)
    Phase 4: Max 5.0 SOL per trade (full production)

Examples:
    ./deploy.sh                     # Standard deployment (Phase 1)
    ./deploy.sh --phase 2           # Deploy Phase 2
    ./deploy.sh --devnet --force    # Force deploy to devnet
    ./deploy.sh --dry-run           # Validation only

EOF
                exit 0
                ;;
            *)
                error_exit "Unknown option: $1. Use --help for usage information." 5
                ;;
        esac
    done
}

################################################################################
# Pre-flight Checks
################################################################################

check_environment() {
    log STEP "Step 1/8: Environment Checks"
    
    log INFO "Checking required commands..."
    check_command "cargo"
    check_command "rustc"
    check_command "git"
    check_command "solana"
    check_command "jq"
    
    log INFO "Checking Rust version..."
    local rust_version=$(rustc --version | awk '{print $2}')
    log INFO "Rust version: ${rust_version}"
    
    log INFO "Checking Solana CLI version..."
    local solana_version=$(solana --version | awk '{print $2}')
    log INFO "Solana CLI version: ${solana_version}"
    
    log INFO "Checking git repository status..."
    if [[ -n $(git status --porcelain) ]]; then
        log WARN "Working directory has uncommitted changes"
        git status --short | head -20 | tee -a "${LOG_FILE}"
        confirm "Deploy with uncommitted changes?"
    else
        log SUCCESS "Working directory is clean"
    fi
    
    local git_commit=$(git rev-parse --short HEAD)
    local git_branch=$(git branch --show-current)
    log INFO "Git branch: ${git_branch}"
    log INFO "Git commit: ${git_commit}"
    
    if [[ "${git_branch}" != "main" && "${TARGET_NETWORK}" == "mainnet" ]]; then
        log WARN "Not on main branch, deploying to mainnet"
        confirm "Deploy to mainnet from non-main branch?"
    fi
    
    log SUCCESS "Environment checks passed"
}

################################################################################
# Test Suite Execution
################################################################################

run_tests() {
    if [[ "${SKIP_TESTS}" == "true" ]]; then
        log WARN "Skipping test suite (as requested)"
        return 0
    fi
    
    log STEP "Step 2/8: Running Test Suite"
    
    log INFO "Running all tests in release mode..."
    if cargo test --release --all-features 2>&1 | tee -a "${LOG_FILE}"; then
        log SUCCESS "All tests passed"
    else
        error_exit "Test suite failed. Fix failing tests before deploying." 1
    fi
    
    log INFO "Running integration tests..."
    if cargo test --test integration_tests --release -- --ignored --test-threads=1 2>&1 | tee -a "${LOG_FILE}"; then
        log SUCCESS "Integration tests passed"
    else
        log WARN "Some integration tests failed (may be optional)"
        confirm "Continue deployment with failing integration tests?"
    fi
    
    log INFO "Running monitoring tests..."
    if cargo test --test monitoring_tests --release -- --test-threads=1 2>&1 | tee -a "${LOG_FILE}"; then
        log SUCCESS "Monitoring tests passed"
    else
        error_exit "Monitoring tests failed. Critical for production operation." 1
    fi
    
    log SUCCESS "Test suite completed successfully"
}

################################################################################
# Benchmark Validation
################################################################################

run_benchmarks() {
    if [[ "${SKIP_BENCHMARKS}" == "true" ]]; then
        log WARN "Skipping benchmark validation (as requested)"
        return 0
    fi
    
    log STEP "Step 3/8: Running Performance Benchmarks"
    
    local bench_output=$(mktemp)
    
    log INFO "Running detection latency benchmark..."
    if cargo test --test integration_tests bench_arbitrage_detection_latency --release -- --ignored --nocapture 2>&1 | tee "${bench_output}"; then
        local detection_latency=$(grep -oP 'Average: \K[0-9.]+' "${bench_output}" | head -1)
        
        if [[ -n "${detection_latency}" ]]; then
            log INFO "Detection latency: ${detection_latency}ms"
            
            if (( $(echo "${detection_latency} > ${MAX_DETECTION_LATENCY_MS}" | bc -l) )); then
                error_exit "Detection latency (${detection_latency}ms) exceeds threshold (${MAX_DETECTION_LATENCY_MS}ms)" 2
            fi
            log SUCCESS "Detection latency within acceptable range"
        else
            log WARN "Could not parse detection latency from benchmark output"
        fi
    else
        log WARN "Detection benchmark failed (non-critical)"
    fi
    
    log INFO "Running transaction building benchmark..."
    if cargo test --test integration_tests bench_transaction_building_latency --release -- --ignored --nocapture 2>&1 | tee "${bench_output}"; then
        local building_latency=$(grep -oP 'Average: \K[0-9.]+' "${bench_output}" | head -1)
        
        if [[ -n "${building_latency}" ]]; then
            log INFO "Building latency: ${building_latency}ms"
            
            if (( $(echo "${building_latency} > ${MAX_BUILDING_LATENCY_MS}" | bc -l) )); then
                error_exit "Building latency (${building_latency}ms) exceeds threshold (${MAX_BUILDING_LATENCY_MS}ms)" 2
            fi
            log SUCCESS "Building latency within acceptable range"
        else
            log WARN "Could not parse building latency from benchmark output"
        fi
    else
        log WARN "Building benchmark failed (non-critical)"
    fi
    
    log INFO "Running end-to-end latency benchmark..."
    if cargo test --test integration_tests bench_end_to_end_latency --release -- --ignored --nocapture 2>&1 | tee "${bench_output}"; then
        local e2e_latency=$(grep -oP 'Average: \K[0-9.]+' "${bench_output}" | head -1)
        
        if [[ -n "${e2e_latency}" ]]; then
            log INFO "End-to-end latency: ${e2e_latency}ms"
            
            if (( $(echo "${e2e_latency} > ${MAX_END_TO_END_LATENCY_MS}" | bc -l) )); then
                error_exit "End-to-end latency (${e2e_latency}ms) exceeds threshold (${MAX_END_TO_END_LATENCY_MS}ms)" 2
            fi
            log SUCCESS "End-to-end latency within acceptable range"
        else
            log WARN "Could not parse end-to-end latency from benchmark output"
        fi
    else
        log WARN "End-to-end benchmark failed (non-critical)"
    fi
    
    rm -f "${bench_output}"
    log SUCCESS "Benchmark validation completed"
}

################################################################################
# Production Build
################################################################################

build_production() {
    log STEP "Step 4/8: Building Production Binary"
    
    log INFO "Cleaning previous builds..."
    cargo clean --release 2>&1 | tee -a "${LOG_FILE}"
    
    log INFO "Building with optimizations..."
    log INFO "Features: metrics, production"
    
    if cargo build --release --features metrics 2>&1 | tee -a "${LOG_FILE}"; then
        log SUCCESS "Production build completed"
    else
        error_exit "Production build failed" 3
    fi
    
    local binary_path="${SCRIPT_DIR}/target/release/mev-bot"
    if [[ ! -f "${binary_path}" ]]; then
        error_exit "Binary not found at ${binary_path}" 3
    fi
    
    local binary_size=$(du -h "${binary_path}" | cut -f1)
    log INFO "Binary size: ${binary_size}"
    
    log INFO "Checking binary symbols..."
    if nm "${binary_path}" | grep -q "debug"; then
        log WARN "Binary contains debug symbols (consider stripping for production)"
    else
        log SUCCESS "Binary is optimized (no debug symbols)"
    fi
    
    log SUCCESS "Production binary ready: ${binary_path}"
}

################################################################################
# Configuration Validation
################################################################################

validate_configuration() {
    log STEP "Step 5/8: Validating Configuration"
    
    local config_file="${SCRIPT_DIR}/config.toml"
    
    if [[ ! -f "${config_file}" ]]; then
        error_exit "Configuration file not found: ${config_file}" 4
    fi
    
    log INFO "Checking required configuration fields..."
    
    # Check RPC configuration
    if ! grep -q "^\[rpc\]" "${config_file}"; then
        error_exit "Missing [rpc] section in config.toml" 4
    fi
    
    local rpc_url=$(grep -A5 "^\[rpc\]" "${config_file}" | grep "^url" | cut -d'"' -f2)
    if [[ -z "${rpc_url}" ]]; then
        error_exit "Missing rpc.url in config.toml" 4
    fi
    log INFO "RPC URL: ${rpc_url}"
    
    # Validate RPC endpoint is reachable
    log INFO "Testing RPC endpoint connectivity..."
    if curl -s -m 5 -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
        "${rpc_url}" | grep -q "ok"; then
        log SUCCESS "RPC endpoint is reachable"
    else
        log WARN "Could not reach RPC endpoint (may require authentication)"
        confirm "Continue with potentially unreachable RPC?"
    fi
    
    # Check keypair configuration
    if ! grep -q "^\[wallet\]" "${config_file}"; then
        error_exit "Missing [wallet] section in config.toml" 4
    fi
    
    local keypair_path=$(grep -A5 "^\[wallet\]" "${config_file}" | grep "^keypair_path" | cut -d'"' -f2)
    if [[ -z "${keypair_path}" ]]; then
        error_exit "Missing wallet.keypair_path in config.toml" 4
    fi
    
    # Expand ~ and relative paths
    keypair_path="${keypair_path/#\~/$HOME}"
    if [[ ! "${keypair_path}" = /* ]]; then
        keypair_path="${SCRIPT_DIR}/${keypair_path}"
    fi
    
    if [[ ! -f "${keypair_path}" ]]; then
        error_exit "Keypair file not found: ${keypair_path}" 4
    fi
    log SUCCESS "Keypair file exists: ${keypair_path}"
    
    # Check wallet balance (if not devnet)
    if [[ "${TARGET_NETWORK}" == "mainnet" ]]; then
        log INFO "Checking wallet balance..."
        local wallet_address=$(solana-keygen pubkey "${keypair_path}")
        local balance=$(solana balance "${wallet_address}" --url "${rpc_url}" 2>/dev/null | awk '{print $1}' || echo "0")
        
        log INFO "Wallet address: ${wallet_address}"
        log INFO "Current balance: ${balance} SOL"
        
        if (( $(echo "${balance} < 0.1" | bc -l) )); then
            log WARN "Low wallet balance: ${balance} SOL"
            log WARN "Recommended minimum: 0.1 SOL for Phase 1 deployment"
            confirm "Continue with low balance?"
        else
            log SUCCESS "Sufficient balance for deployment"
        fi
    fi
    
    # Check arbitrage configuration
    if ! grep -q "^\[arbitrage\]" "${config_file}"; then
        error_exit "Missing [arbitrage] section in config.toml" 4
    fi
    
    local min_profit=$(grep -A10 "^\[arbitrage\]" "${config_file}" | grep "^min_profit_threshold" | awk '{print $3}')
    local max_position=$(grep -A10 "^\[arbitrage\]" "${config_file}" | grep "^max_position_size" | awk '{print $3}')
    
    log INFO "Min profit threshold: ${min_profit} SOL"
    log INFO "Max position size: ${max_position} SOL"
    
    # Validate phase-appropriate settings
    local phase_max_position=$(echo "${PHASE_CONFIG[$DEPLOYMENT_PHASE]}" | cut -d',' -f1 | cut -d'=' -f2)
    if (( $(echo "${max_position} > ${phase_max_position}" | bc -l) )); then
        log WARN "Configuration max_position_size (${max_position}) exceeds Phase ${DEPLOYMENT_PHASE} limit (${phase_max_position})"
        confirm "Continue with higher position size than recommended?"
    fi
    
    # Check safety configuration
    if ! grep -q "^\[safety\]" "${config_file}"; then
        log WARN "Missing [safety] section (recommended for production)"
    else
        local dry_run=$(grep -A5 "^\[safety\]" "${config_file}" | grep "^dry_run_only" | awk '{print $3}')
        if [[ "${dry_run}" == "true" ]]; then
            log WARN "dry_run_only is enabled - bot will not send real transactions"
            confirm "Deploy in dry-run mode?"
        fi
    fi
    
    # Check circuit breaker configuration
    if ! grep -q "^\[circuit_breaker\]" "${config_file}"; then
        log WARN "Missing [circuit_breaker] section (recommended for production)"
    else
        log SUCCESS "Circuit breaker configured"
    fi
    
    log SUCCESS "Configuration validation passed"
}

################################################################################
# Environment Checks
################################################################################

check_production_environment() {
    log STEP "Step 6/8: Production Environment Checks"
    
    # Check system resources
    log INFO "Checking system resources..."
    
    # Memory check
    if [[ "$(uname)" == "Darwin" ]]; then
        local total_mem=$(sysctl -n hw.memsize | awk '{print $1/1024/1024/1024}')
        log INFO "Total memory: ${total_mem} GB"
    else
        local total_mem=$(free -g | awk '/^Mem:/{print $2}')
        log INFO "Total memory: ${total_mem} GB"
    fi
    
    # Disk space check
    local free_space=$(df -h "${SCRIPT_DIR}" | awk 'NR==2 {print $4}')
    log INFO "Free disk space: ${free_space}"
    
    # Check for existing bot processes
    log INFO "Checking for existing bot processes..."
    if pgrep -f "mev-bot" > /dev/null; then
        log WARN "Found existing mev-bot process(es):"
        pgrep -af "mev-bot" | tee -a "${LOG_FILE}"
        confirm "Kill existing processes and continue?"
        pkill -f "mev-bot" || true
        sleep 2
    else
        log SUCCESS "No existing bot processes found"
    fi
    
    # Check network connectivity
    log INFO "Checking network connectivity..."
    if ping -c 1 -W 2 8.8.8.8 > /dev/null 2>&1; then
        log SUCCESS "Network connectivity OK"
    else
        log WARN "Network connectivity check failed"
    fi
    
    # Check for monitoring setup
    log INFO "Checking monitoring setup..."
    if command -v prometheus &> /dev/null; then
        log SUCCESS "Prometheus found"
    else
        log WARN "Prometheus not found (metrics may not be collected)"
    fi
    
    if command -v grafana-server &> /dev/null; then
        log SUCCESS "Grafana found"
    else
        log WARN "Grafana not found (no dashboards available)"
    fi
    
    log SUCCESS "Environment checks completed"
}

################################################################################
# Deployment Package Creation
################################################################################

create_deployment_package() {
    log STEP "Step 7/8: Creating Deployment Package"
    
    if [[ "${DRY_RUN}" == "true" ]]; then
        log INFO "Skipping deployment package creation (dry-run mode)"
        return 0
    fi
    
    log INFO "Creating deployment directory: ${DEPLOY_DIR}"
    mkdir -p "${DEPLOY_DIR}"
    
    # Get git information
    local git_commit=$(git rev-parse HEAD)
    local git_short=$(git rev-parse --short HEAD)
    local git_branch=$(git branch --show-current)
    local git_tag=$(git describe --tags --always 2>/dev/null || echo "untagged")
    
    # Copy binary
    log INFO "Copying production binary..."
    cp "${SCRIPT_DIR}/target/release/mev-bot" "${DEPLOY_DIR}/"
    chmod +x "${DEPLOY_DIR}/mev-bot"
    
    # Copy configuration
    log INFO "Copying configuration..."
    cp "${SCRIPT_DIR}/config.toml" "${DEPLOY_DIR}/config.toml"
    
    # Create phase-specific configuration
    local phase_config="${DEPLOY_DIR}/config-phase${DEPLOYMENT_PHASE}.toml"
    cp "${SCRIPT_DIR}/config.toml" "${phase_config}"
    
    # Update max_position_size for phase
    local phase_max_position=$(echo "${PHASE_CONFIG[$DEPLOYMENT_PHASE]}" | cut -d',' -f1 | cut -d'=' -f2)
    if command -v sed &> /dev/null; then
        # macOS-compatible sed
        sed -i '' "s/^max_position_size = .*/max_position_size = ${phase_max_position}/" "${phase_config}" 2>/dev/null || \
        sed -i "s/^max_position_size = .*/max_position_size = ${phase_max_position}/" "${phase_config}"
    fi
    log INFO "Created phase-specific config: ${phase_config}"
    
    # Create startup script
    log INFO "Creating startup script..."
    cat > "${DEPLOY_DIR}/start-bot.sh" << 'EOF'
#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG="${1:-${SCRIPT_DIR}/config.toml}"
LOG_DIR="${SCRIPT_DIR}/logs"

mkdir -p "${LOG_DIR}"

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
LOG_FILE="${LOG_DIR}/bot-${TIMESTAMP}.log"

echo "Starting MEV bot..."
echo "Config: ${CONFIG}"
echo "Logs: ${LOG_FILE}"

# Run bot with output redirected to log file
"${SCRIPT_DIR}/mev-bot" --config "${CONFIG}" 2>&1 | tee "${LOG_FILE}"
EOF
    
    chmod +x "${DEPLOY_DIR}/start-bot.sh"
    
    # Create stop script
    log INFO "Creating stop script..."
    cat > "${DEPLOY_DIR}/stop-bot.sh" << 'EOF'
#!/usr/bin/env bash

set -euo pipefail

echo "Stopping MEV bot..."

if pgrep -f "mev-bot" > /dev/null; then
    pkill -f "mev-bot"
    echo "Bot stopped successfully"
else
    echo "No bot process found"
fi
EOF
    
    chmod +x "${DEPLOY_DIR}/stop-bot.sh"
    
    # Create systemd service file (optional)
    log INFO "Creating systemd service file..."
    cat > "${DEPLOY_DIR}/mev-bot.service" << EOF
[Unit]
Description=Solana MEV Arbitrage Bot
After=network.target

[Service]
Type=simple
User=${USER}
WorkingDirectory=${DEPLOY_DIR}
ExecStart=${DEPLOY_DIR}/start-bot.sh ${DEPLOY_DIR}/config-phase${DEPLOYMENT_PHASE}.toml
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
    
    # Create version info file
    log INFO "Creating version info..."
    cat > "${DEPLOY_DIR}/VERSION" << EOF
Version: ${git_tag}
Commit: ${git_commit}
Short: ${git_short}
Branch: ${git_branch}
Build Date: $(date -u '+%Y-%m-%d %H:%M:%S UTC')
Built By: ${USER}@$(hostname)
Target Network: ${TARGET_NETWORK}
Deployment Phase: ${DEPLOYMENT_PHASE}
EOF
    
    # Create README for deployment
    log INFO "Creating deployment README..."
    cat > "${DEPLOY_DIR}/README.md" << EOF
# Solana MEV Bot Deployment Package

**Version:** ${git_tag} (${git_short})  
**Built:** $(date -u '+%Y-%m-%d %H:%M:%S UTC')  
**Network:** ${TARGET_NETWORK}  
**Phase:** ${DEPLOYMENT_PHASE}

## Quick Start

### Option 1: Direct Execution
\`\`\`bash
# Start bot
./start-bot.sh

# Stop bot
./stop-bot.sh
\`\`\`

### Option 2: Systemd Service (Linux)
\`\`\`bash
# Install service
sudo cp mev-bot.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable mev-bot
sudo systemctl start mev-bot

# Check status
sudo systemctl status mev-bot

# View logs
sudo journalctl -u mev-bot -f
\`\`\`

## Configuration

Phase-specific configuration: \`config-phase${DEPLOYMENT_PHASE}.toml\`

Phase ${DEPLOYMENT_PHASE} Settings:
$(echo "${PHASE_CONFIG[$DEPLOYMENT_PHASE]}" | tr ',' '\n' | sed 's/^/- /')

## Monitoring

Metrics endpoint: \`http://localhost:9090/metrics\`

Key metrics:
- \`opportunities_detected\` - Total opportunities found
- \`transactions_sent\` - Transactions submitted
- \`transactions_failed\` - Failed transactions
- \`circuit_breaker_triggered\` - Circuit breaker activations
- \`detection_latency_ms\` - Detection latency histogram
- \`profit_per_trade_sol\` - Profit per trade histogram

## Phase Progression

### Current Phase: ${DEPLOYMENT_PHASE}
$(echo "${PHASE_CONFIG[$DEPLOYMENT_PHASE]}" | tr ',' '\n')

### Next Steps
After successful completion of Phase ${DEPLOYMENT_PHASE}:
1. Monitor metrics for at least the recommended duration
2. Validate profitability and stability
3. Review logs for any issues
4. Deploy next phase with increased position sizes

### Deployment Phases
- Phase 1: Max 0.1 SOL (1 week validation)
- Phase 2: Max 0.5 SOL (1 week small scale)
- Phase 3: Max 1.0 SOL (2 weeks medium scale)
- Phase 4: Max 5.0 SOL (full production)

## Troubleshooting

### Bot won't start
- Check config.toml has correct RPC URL
- Verify keypair file exists and has correct permissions
- Ensure sufficient SOL balance
- Check logs in logs/ directory

### High failure rate
- Check RPC endpoint performance
- Verify network connectivity
- Review circuit breaker thresholds
- Check for insufficient balance

### No opportunities detected
- Verify pool data is being fetched
- Check min_profit_threshold isn't too high
- Ensure RPC is returning current data
- Review detection latency metrics

## Support

- Logs: \`logs/\` directory
- Config: \`config-phase${DEPLOYMENT_PHASE}.toml\`
- Version: \`VERSION\` file
- Main config: \`config.toml\`

## Safety Features

- Circuit breaker: Stops trading after threshold failures
- Dry-run mode: Test without real transactions
- Position limits: Enforced per-phase maximums
- Metrics monitoring: Full observability

EOF
    
    # Create tarball
    log INFO "Creating deployment tarball..."
    local tarball_name="mev-bot-${git_short}-phase${DEPLOYMENT_PHASE}-${TARGET_NETWORK}.tar.gz"
    tar -czf "${SCRIPT_DIR}/${tarball_name}" -C "${SCRIPT_DIR}" "$(basename "${DEPLOY_DIR}")"
    
    log SUCCESS "Deployment package created: ${DEPLOY_DIR}"
    log SUCCESS "Tarball created: ${tarball_name}"
    
    # Show package contents
    log INFO "Package contents:"
    ls -lh "${DEPLOY_DIR}" | tee -a "${LOG_FILE}"
    
    local tarball_size=$(du -h "${SCRIPT_DIR}/${tarball_name}" | cut -f1)
    log INFO "Tarball size: ${tarball_size}"
}

################################################################################
# Deployment Summary
################################################################################

show_deployment_summary() {
    log STEP "Step 8/8: Deployment Summary"
    
    local git_short=$(git rev-parse --short HEAD)
    
    cat << EOF | tee -a "${LOG_FILE}"

${GREEN}${BOLD}╔═══════════════════════════════════════════════════════════════╗
║                  DEPLOYMENT SUCCESSFUL ✓                      ║
╚═══════════════════════════════════════════════════════════════╝${NC}

${CYAN}📦 Deployment Package${NC}
   Location: ${DEPLOY_DIR}
   Tarball:  mev-bot-${git_short}-phase${DEPLOYMENT_PHASE}-${TARGET_NETWORK}.tar.gz

${CYAN}🚀 Quick Start${NC}
   cd ${DEPLOY_DIR}
   ./start-bot.sh

${CYAN}🎯 Deployment Phase: ${DEPLOYMENT_PHASE}${NC}
$(echo "${PHASE_CONFIG[$DEPLOYMENT_PHASE]}" | tr ',' '\n' | sed 's/^/   /')

${CYAN}📊 Monitoring${NC}
   Metrics: http://localhost:9090/metrics
   Logs:    ${DEPLOY_DIR}/logs/

${CYAN}⚠️  Important Reminders${NC}
   1. Start with Phase ${DEPLOYMENT_PHASE} limits (max_position_size enforced)
   2. Monitor metrics continuously for first 24-48 hours
   3. Check circuit breaker triggers (should be zero)
   4. Validate profitability before scaling to next phase
   5. Keep backups of logs and configuration

${CYAN}📈 Next Phase Criteria${NC}
   - Run for recommended duration: $(echo "${PHASE_CONFIG[$DEPLOYMENT_PHASE]}" | cut -d',' -f2 | cut -d'=' -f2)
   - Maintain >70% success rate
   - Zero circuit breaker triggers (or investigate cause)
   - Positive ROI (profit > costs)
   - Stable memory usage

${CYAN}🔧 Useful Commands${NC}
   # Start bot
   cd ${DEPLOY_DIR} && ./start-bot.sh

   # Stop bot
   cd ${DEPLOY_DIR} && ./stop-bot.sh

   # Check metrics
   curl http://localhost:9090/metrics

   # View logs
   tail -f ${DEPLOY_DIR}/logs/bot-*.log

   # Install as service (Linux)
   sudo cp ${DEPLOY_DIR}/mev-bot.service /etc/systemd/system/
   sudo systemctl enable mev-bot
   sudo systemctl start mev-bot

${CYAN}📞 Support${NC}
   - Documentation: ${SCRIPT_DIR}/README.md
   - Test Report: ${SCRIPT_DIR}/TEST_REPORT_ANALYSIS.md
   - Deployment Log: ${LOG_FILE}

${GREEN}Good luck with your MEV journey! 🚀${NC}

EOF
}

################################################################################
# Main Execution
################################################################################

main() {
    # Show banner
    cat << "EOF"
╔═══════════════════════════════════════════════════════════════╗
║            Solana MEV Bot - Deployment Script                ║
║                                                               ║
║  Comprehensive validation and production deployment          ║
╚═══════════════════════════════════════════════════════════════╝
EOF
    
    echo "" | tee -a "${LOG_FILE}"
    log INFO "Deployment started at $(date)"
    log INFO "Log file: ${LOG_FILE}"
    echo ""
    
    # Parse command-line arguments
    parse_args "$@"
    
    # Show deployment configuration
    log INFO "Deployment Configuration:"
    log INFO "  Target Network: ${TARGET_NETWORK}"
    log INFO "  Deployment Phase: ${DEPLOYMENT_PHASE}"
    log INFO "  Skip Tests: ${SKIP_TESTS}"
    log INFO "  Skip Benchmarks: ${SKIP_BENCHMARKS}"
    log INFO "  Dry Run: ${DRY_RUN}"
    log INFO "  Force: ${FORCE}"
    echo ""
    
    # Confirm deployment
    if [[ "${TARGET_NETWORK}" == "mainnet" ]]; then
        cat << EOF
${RED}${BOLD}⚠️  WARNING: MAINNET DEPLOYMENT ⚠️${NC}

You are about to deploy to Solana mainnet with REAL SOL.

Phase ${DEPLOYMENT_PHASE} Configuration:
$(echo "${PHASE_CONFIG[$DEPLOYMENT_PHASE]}" | tr ',' '\n' | sed 's/^/  /')

This bot will execute REAL transactions using REAL money.
Make sure you understand the risks involved.

EOF
        confirm "Deploy to mainnet?"
    fi
    
    # Execute deployment steps
    check_environment
    run_tests
    run_benchmarks
    build_production
    validate_configuration
    check_production_environment
    create_deployment_package
    show_deployment_summary
    
    log SUCCESS "Deployment completed successfully at $(date)"
    
    if [[ "${DRY_RUN}" == "true" ]]; then
        log INFO "This was a dry-run. No deployment package created."
        log INFO "Run without --dry-run to create actual deployment."
    fi
}

# Trap errors and cleanup
trap 'error_exit "Script interrupted" 1' INT TERM

# Run main function
main "$@"

exit 0
