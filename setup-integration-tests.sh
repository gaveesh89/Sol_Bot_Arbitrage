#!/usr/bin/env bash
# Setup script for integration tests with mainnet fork

set -e

echo "🔧 Setting up Solana Mainnet Fork Integration Tests"
echo ""

# Check for solana-test-validator
echo "📋 Checking prerequisites..."
if command -v solana-test-validator &> /dev/null; then
    echo "✅ solana-test-validator found: $(solana-test-validator --version | head -1)"
else
    echo "❌ solana-test-validator not found"
    echo "   Install Solana CLI tools:"
    echo "   sh -c \"\$(curl -sSfL https://release.solana.com/stable/install)\""
    exit 1
fi

# Check for protobuf compiler (needed for etcd-client dependency)
if command -v protoc &> /dev/null; then
    echo "✅ protoc found: $(protoc --version)"
else
    echo "⚠️  protoc not found - installing..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "   Installing via Homebrew..."
        brew install protobuf
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "   Installing via apt..."
        sudo apt-get update && sudo apt-get install -y protobuf-compiler
    else
        echo "❌ Unsupported OS. Please install protobuf manually:"
        echo "   https://github.com/protocolbuffers/protobuf/releases"
        exit 1
    fi
fi

# Check for API keys
echo ""
echo "🔑 Checking API keys..."

if [ -z "$HELIUS_API_KEY" ]; then
    echo "⚠️  HELIUS_API_KEY not set"
    echo "   Get your key at: https://helius.dev"
    echo "   Then run: export HELIUS_API_KEY=\"your_key_here\""
    
    if [ -f ".env" ]; then
        echo "   Or add to .env file (already exists)"
    else
        echo "   Creating .env file template..."
        cat > .env << 'EOF'
# Helius RPC API Key
# Get yours at: https://helius.dev
HELIUS_API_KEY=your_helius_api_key_here

# Optional: Solscan API Key
# Get yours at: https://solscan.io
SOLSCAN_API_KEY=your_solscan_api_key_here
EOF
        echo "   Created .env file - please edit and add your keys"
    fi
else
    echo "✅ HELIUS_API_KEY is set: ${HELIUS_API_KEY:0:8}..."
fi

# Build the project
echo ""
echo "🔨 Building project..."
cargo build --tests 2>&1 | tail -20

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
else
    echo "❌ Build failed - check errors above"
    exit 1
fi

# List available tests
echo ""
echo "📋 Available integration tests:"
cargo test --test integration_tests -- --list --ignored 2>/dev/null | grep "test_" | sed 's/: test//'

echo ""
echo "✅ Setup complete!"
echo ""
echo "🚀 Run tests with:"
echo "   ./run-integration-tests.sh                    # Run all tests"
echo "   ./run-integration-tests.sh test_name          # Run specific test"
echo ""
echo "Or manually:"
echo "   cargo test --test integration_tests -- --test-threads=1 --nocapture --ignored"
