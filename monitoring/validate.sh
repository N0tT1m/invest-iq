#!/bin/bash
# Monitoring Infrastructure Validation Script
# Tests that all monitoring components are properly configured and running

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=========================================="
echo "InvestIQ Monitoring Validation"
echo "=========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
TESTS_PASSED=0
TESTS_FAILED=0

# Test function
test_check() {
    local test_name="$1"
    local test_command="$2"

    echo -n "Testing: $test_name... "

    if eval "$test_command" > /dev/null 2>&1; then
        echo -e "${GREEN}PASS${NC}"
        ((TESTS_PASSED++))
        return 0
    else
        echo -e "${RED}FAIL${NC}"
        ((TESTS_FAILED++))
        return 1
    fi
}

# Configuration validation
echo "==> Configuration Files"
test_check "Prometheus config exists" "[ -f '$SCRIPT_DIR/prometheus.yml' ]"
test_check "Alert rules exist" "[ -f '$SCRIPT_DIR/alert_rules.yml' ]"
test_check "Grafana datasource config" "[ -f '$SCRIPT_DIR/grafana/provisioning/datasources/prometheus.yml' ]"
test_check "Grafana dashboard provisioning" "[ -f '$SCRIPT_DIR/grafana/provisioning/dashboards/dashboard.yml' ]"
test_check "InvestIQ dashboard JSON" "[ -f '$SCRIPT_DIR/grafana/dashboards/investiq-overview.json' ]"
echo ""

# Syntax validation
echo "==> Syntax Validation"

# Validate YAML files
if command -v yamllint &> /dev/null; then
    test_check "Prometheus YAML syntax" "yamllint -d relaxed '$SCRIPT_DIR/prometheus.yml'"
    test_check "Alert rules YAML syntax" "yamllint -d relaxed '$SCRIPT_DIR/alert_rules.yml'"
else
    echo -e "${YELLOW}SKIP: yamllint not installed${NC}"
fi

# Validate JSON dashboard
if command -v jq &> /dev/null; then
    test_check "Dashboard JSON syntax" "jq empty '$SCRIPT_DIR/grafana/dashboards/investiq-overview.json'"
else
    echo -e "${YELLOW}SKIP: jq not installed${NC}"
fi
echo ""

# Docker compose validation
echo "==> Docker Compose Configuration"
cd "$PROJECT_ROOT"
test_check "Docker compose file exists" "[ -f 'docker-compose.yml' ]"
test_check "Prometheus service defined" "grep -q 'prometheus:' docker-compose.yml"
test_check "Grafana service defined" "grep -q 'grafana:' docker-compose.yml"
test_check "Monitoring profile defined" "grep -q 'profile.*monitoring' docker-compose.yml"
test_check "Prometheus volume defined" "grep -q 'prometheus_data:' docker-compose.yml"
test_check "Grafana volume defined" "grep -q 'grafana_data:' docker-compose.yml"
echo ""

# Runtime checks (if services are running)
echo "==> Runtime Checks"

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo -e "${YELLOW}Docker is not running - skipping runtime checks${NC}"
else
    # Check if monitoring services are running
    if docker ps | grep -q investiq-prometheus; then
        test_check "Prometheus container running" "docker ps | grep -q investiq-prometheus"

        # Test Prometheus endpoint
        if command -v curl &> /dev/null; then
            test_check "Prometheus API accessible" "curl -sf http://localhost:9090/-/healthy"
            test_check "Prometheus config valid" "curl -sf http://localhost:9090/api/v1/status/config | grep -q 'scrape_configs'"
        fi
    else
        echo -e "${YELLOW}Prometheus not running - start with: docker compose --profile monitoring up -d${NC}"
    fi

    if docker ps | grep -q investiq-grafana; then
        test_check "Grafana container running" "docker ps | grep -q investiq-grafana"

        # Test Grafana endpoint
        if command -v curl &> /dev/null; then
            test_check "Grafana API accessible" "curl -sf http://localhost:3001/api/health"
        fi
    else
        echo -e "${YELLOW}Grafana not running - start with: docker compose --profile monitoring up -d${NC}"
    fi

    # Check if API server is exposing metrics
    if docker ps | grep -q investiq-api; then
        if command -v curl &> /dev/null; then
            test_check "API server metrics endpoint" "curl -sf http://localhost:3000/metrics | grep -q 'investiq'"
        fi
    else
        echo -e "${YELLOW}API server not running${NC}"
    fi
fi
echo ""

# Summary
echo "=========================================="
echo "Validation Summary"
echo "=========================================="
echo -e "Tests Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests Failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All validation checks passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some validation checks failed${NC}"
    echo ""
    echo "Troubleshooting:"
    echo "1. Ensure all configuration files are present"
    echo "2. Check YAML/JSON syntax with yamllint/jq"
    echo "3. Start monitoring stack: docker compose --profile monitoring up -d"
    echo "4. View logs: docker logs investiq-prometheus && docker logs investiq-grafana"
    exit 1
fi
