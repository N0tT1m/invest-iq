#!/bin/bash

# Deploy InvestIQ Autonomous Trading Agent to GPU Machine
# Usage: ./deploy-to-gpu-machine.sh [user@gpu-machine] [path]

set -e

GPU_HOST="${1:-user@gpu-machine}"
REMOTE_PATH="${2:-/home/user/trading/invest-iq}"

echo "🚀 Deploying InvestIQ Trading Agent to GPU Machine"
echo "=================================================="
echo "Target: $GPU_HOST:$REMOTE_PATH"
echo ""

# Step 1: Build locally
echo "📦 Building release binaries..."
cargo build --release --bin trading-agent
cargo build --release --bin api-server

# Step 2: Create deployment package
echo "📦 Creating deployment package..."
tar -czf /tmp/invest-iq-deploy.tar.gz \
    --exclude='target/debug' \
    --exclude='target/*/deps' \
    --exclude='target/*/build' \
    --exclude='.git' \
    --exclude='node_modules' \
    --exclude='venv' \
    --exclude='*.log' \
    target/release/trading-agent \
    target/release/api-server \
    .env.example \
    Cargo.toml \
    Cargo.lock \
    crates/ \
    frontend/*.py \
    schema.sql \
    AUTONOMOUS_TRADING_SETUP.md \
    README.md

# Step 3: Upload to GPU machine
echo "📤 Uploading to GPU machine..."
ssh "$GPU_HOST" "mkdir -p $REMOTE_PATH"
scp /tmp/invest-iq-deploy.tar.gz "$GPU_HOST:$REMOTE_PATH/"

# Step 4: Extract and setup
echo "📦 Extracting on remote machine..."
ssh "$GPU_HOST" << EOF
    cd $REMOTE_PATH
    tar -xzf invest-iq-deploy.tar.gz
    rm invest-iq-deploy.tar.gz

    # Create .env.trading if doesn't exist
    if [ ! -f .env.trading ]; then
        echo "Creating .env.trading from example..."
        cp .env.example .env.trading
        echo ""
        echo "⚠️  IMPORTANT: Edit .env.trading with your API keys!"
        echo "    nano $REMOTE_PATH/.env.trading"
    fi

    echo "✅ Deployment complete!"
    echo ""
    echo "Next steps:"
    echo "1. Edit $REMOTE_PATH/.env.trading with your API keys"
    echo "2. Install Ollama: curl -fsSL https://ollama.com/install.sh | sh"
    echo "3. Pull model: ollama pull llama3.1:70b"
    echo "4. Test run: cd $REMOTE_PATH && ./target/release/trading-agent"
    echo ""
    echo "For full setup instructions, see:"
    echo "    $REMOTE_PATH/AUTONOMOUS_TRADING_SETUP.md"
EOF

# Cleanup
rm /tmp/invest-iq-deploy.tar.gz

echo ""
echo "✅ Deployment complete!"
echo ""
echo "🎯 SSH to your GPU machine and follow the setup guide:"
echo "   ssh $GPU_HOST"
echo "   cd $REMOTE_PATH"
echo "   cat AUTONOMOUS_TRADING_SETUP.md"
