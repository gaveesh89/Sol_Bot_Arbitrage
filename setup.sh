#!/bin/bash

# Solana MEV Bot Setup Script

set -e

echo "🚀 Setting up Solana MEV Bot..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "✅ Rust detected: $(rustc --version)"

# Check if Solana CLI is installed (optional but recommended)
if command -v solana &> /dev/null; then
    echo "✅ Solana CLI detected: $(solana --version)"
else
    echo "⚠️  Solana CLI not found (optional)"
    echo "   Install from: https://docs.solana.com/cli/install-solana-cli-tools"
fi

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    echo "📝 Creating .env file from .env.example..."
    cp .env.example .env
    echo "✅ .env file created"
    echo "⚠️  Please edit .env with your configuration before running the bot"
else
    echo "✅ .env file already exists"
fi

# Create logs directory
mkdir -p logs
echo "✅ Created logs directory"

# Check if wallet exists
if [ ! -f wallet.json ]; then
    echo "⚠️  No wallet.json found"
    echo "   Options:"
    echo "   1. Generate new wallet: solana-keygen new -o wallet.json"
    echo "   2. Copy existing wallet to wallet.json"
    echo "   3. Set WALLET_PRIVATE_KEY in .env"
else
    echo "✅ wallet.json found"
fi

# Build the project
echo "🔨 Building the project (this may take a while)..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo ""
    echo "📋 Next steps:"
    echo "   1. Edit .env with your RPC endpoint and configuration"
    echo "   2. Ensure your wallet has SOL for transaction fees"
    echo "   3. Add pool addresses to monitor in src/main.rs"
    echo "   4. Run the bot: cargo run --release"
    echo ""
    echo "⚠️  IMPORTANT: Test thoroughly on devnet before using on mainnet!"
else
    echo "❌ Build failed. Please check the errors above."
    exit 1
fi
