#!/bin/bash
# InvestIQ Secrets Setup Script
# Creates secret files for Docker Compose secrets support

set -euo pipefail

SECRETS_DIR="./secrets"
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "========================================="
echo "  InvestIQ Secrets Setup"
echo "========================================="
echo ""

# Create secrets directory if it doesn't exist
if [ ! -d "$SECRETS_DIR" ]; then
    echo -e "${YELLOW}Creating secrets directory...${NC}"
    mkdir -p "$SECRETS_DIR"
fi

# Function to create secret file
create_secret() {
    local name=$1
    local prompt=$2
    local optional=${3:-false}
    local file_path="${SECRETS_DIR}/${name}.txt"

    # Check if file already exists
    if [ -f "$file_path" ]; then
        echo -e "${YELLOW}Secret already exists: ${name}${NC}"
        read -p "Overwrite? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${GREEN}Keeping existing secret${NC}"
            return
        fi
    fi

    # Prompt for secret value
    echo -e "${YELLOW}${prompt}${NC}"
    if [ "$optional" = true ]; then
        echo "(Optional - press Enter to skip)"
    fi

    read -r -s secret_value
    echo

    # Skip if empty and optional
    if [ -z "$secret_value" ] && [ "$optional" = true ]; then
        echo -e "${YELLOW}Skipped (optional)${NC}"
        return
    fi

    # Validate not empty for required secrets
    if [ -z "$secret_value" ]; then
        echo -e "${RED}Error: Required secret cannot be empty${NC}"
        exit 1
    fi

    # Write secret to file (no trailing newline)
    echo -n "$secret_value" > "$file_path"
    chmod 600 "$file_path"

    echo -e "${GREEN}✓ Created: ${file_path}${NC}"
}

# Function to generate random API key
generate_api_key() {
    if command -v openssl &> /dev/null; then
        openssl rand -hex 32
    else
        # Fallback if openssl not available
        cat /dev/urandom | LC_ALL=C tr -dc 'a-f0-9' | fold -w 64 | head -n 1
    fi
}

echo "This script will create secret files for Docker Compose."
echo "Secrets will be stored in: ${SECRETS_DIR}"
echo ""
echo -e "${YELLOW}IMPORTANT:${NC}"
echo "  - Secret files are excluded from git"
echo "  - Files will have 600 permissions (owner read/write only)"
echo "  - Use environment variables for development (secrets are for production)"
echo ""
read -p "Continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 0
fi

echo ""
echo "========================================="
echo "  Required Secrets"
echo "========================================="
echo ""

# Polygon API Key
create_secret "polygon_api_key" \
    "Enter Polygon API key (from https://polygon.io/dashboard):"

# Alpaca API Key
create_secret "alpaca_api_key" \
    "Enter Alpaca API key ID (from https://app.alpaca.markets):"

# Alpaca Secret Key
create_secret "alpaca_secret_key" \
    "Enter Alpaca secret key (from https://app.alpaca.markets):"

# API Keys (can be auto-generated)
echo ""
echo -e "${YELLOW}Generate InvestIQ API keys?${NC}"
echo "You can provide your own keys or auto-generate them."
read -p "Auto-generate? (Y/n): " -n 1 -r
echo
if [[ $REPLY =~ ^[Nn]$ ]]; then
    create_secret "api_keys" \
        "Enter API keys (format: key1:admin,key2:trader):"
else
    ADMIN_KEY=$(generate_api_key)
    TRADER_KEY=$(generate_api_key)
    echo -n "${ADMIN_KEY}:admin,${TRADER_KEY}:trader" > "${SECRETS_DIR}/api_keys.txt"
    chmod 600 "${SECRETS_DIR}/api_keys.txt"
    echo -e "${GREEN}✓ Generated API keys${NC}"
    echo ""
    echo -e "${YELLOW}Admin key:${NC}  ${ADMIN_KEY}"
    echo -e "${YELLOW}Trader key:${NC} ${TRADER_KEY}"
    echo ""
    echo -e "${RED}SAVE THESE KEYS! They will not be displayed again.${NC}"
    echo ""
fi

echo ""
echo "========================================="
echo "  Optional Secrets"
echo "========================================="
echo ""

# Live Trading Key (optional)
echo -e "${YELLOW}Generate live trading key?${NC}"
echo "This enables broker write operations (trades, orders)."
read -p "Generate? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    LIVE_KEY=$(generate_api_key)
    echo -n "$LIVE_KEY" > "${SECRETS_DIR}/live_trading_key.txt"
    chmod 600 "${SECRETS_DIR}/live_trading_key.txt"
    echo -e "${GREEN}✓ Generated live trading key${NC}"
    echo -e "${YELLOW}Live trading key:${NC} ${LIVE_KEY}"
    echo ""
    echo -e "${RED}SAVE THIS KEY! It will not be displayed again.${NC}"
    echo -e "${YELLOW}Update your frontend .env with: LIVE_TRADING_KEY=${LIVE_KEY}${NC}"
    echo ""
fi

# Finnhub API Key (optional)
create_secret "finnhub_api_key" \
    "Enter Finnhub API key (from https://finnhub.io/dashboard):" \
    true

echo ""
echo "========================================="
echo "  Setup Complete!"
echo "========================================="
echo ""

# List created secrets
echo "Created secrets:"
for file in "${SECRETS_DIR}"/*.txt; do
    if [ -f "$file" ]; then
        size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
        echo -e "  ${GREEN}✓${NC} $(basename "$file") (${size} bytes)"
    fi
done

echo ""
echo "Next steps:"
echo "  1. Verify secrets are correct:"
echo "     ls -la ${SECRETS_DIR}/"
echo ""
echo "  2. Deploy with Docker Compose:"
echo "     docker compose up -d"
echo ""
echo "  3. For staging environment:"
echo "     docker compose -f docker-compose.yml -f docker-compose.staging.yml up -d"
echo ""
echo "  4. For production with TLS:"
echo "     export DOMAIN=api.yourdomain.com"
echo "     export ACME_EMAIL=admin@yourdomain.com"
echo "     docker compose -f docker-compose.yml -f docker-compose.production.yml up -d"
echo ""
echo -e "${YELLOW}Security reminder:${NC}"
echo "  - Never commit secret files to git"
echo "  - Rotate secrets every 90 days"
echo "  - See docs/secrets-rotation.md for rotation procedures"
echo ""
