#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== LLM over DNS - Code Coverage Report ===${NC}\n"

# Check if cargo-tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}cargo-tarpaulin not found. Installing...${NC}"
    cargo install cargo-tarpaulin
fi

echo -e "${BLUE}Running tests with coverage tracking...${NC}\n"

# Create coverage directory
mkdir -p coverage

# Run tarpaulin with HTML output
cargo tarpaulin \
    --out Html \
    --output-dir coverage \
    --timeout 120 \
    --exclude-files build.rs \
    --verbose

# Check if coverage report was generated
if [ -f "coverage/index.html" ]; then
    echo -e "\n${GREEN}✓ Coverage report generated successfully${NC}"
    echo -e "${BLUE}Report location: ${PWD}/coverage/index.html${NC}\n"

    # Parse coverage percentage from the report
    COVERAGE=$(grep -oP 'Coverage: \K[0-9.]+(?=%)' coverage/index.html | head -1)

    if [ -n "$COVERAGE" ]; then
        echo -e "${BLUE}Overall Coverage: ${COVERAGE}%${NC}"

        # Check if coverage meets target (100%)
        if (( $(echo "$COVERAGE >= 100" | bc -l) )); then
            echo -e "${GREEN}✓ Coverage target (100%) met!${NC}\n"
            exit 0
        else
            SHORTFALL=$(echo "100 - $COVERAGE" | bc -l)
            echo -e "${YELLOW}⚠ Coverage is ${SHORTFALL}% below target${NC}\n"
            exit 1
        fi
    fi
else
    echo -e "${RED}✗ Coverage report generation failed${NC}\n"
    exit 1
fi
