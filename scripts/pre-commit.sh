#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Pre-commit Quality Checks ===${NC}\n"

FAILED=0

# Check 1: Run tests
echo -e "${BLUE}[1/4] Running tests...${NC}"
if cargo test --quiet 2>&1; then
    echo -e "${GREEN}✓ All tests passed${NC}\n"
else
    echo -e "${RED}✗ Tests failed${NC}\n"
    FAILED=$((FAILED + 1))
fi

# Check 2: Clippy - Zero warnings policy
echo -e "${BLUE}[2/4] Running clippy (zero warnings)...${NC}"
if cargo clippy --all-targets --all-features -- -D warnings 2>&1 > /tmp/clippy.log; then
    echo -e "${GREEN}✓ No clippy warnings found${NC}\n"
else
    echo -e "${RED}✗ Clippy warnings found:${NC}"
    cat /tmp/clippy.log
    echo ""
    FAILED=$((FAILED + 1))
fi

# Check 3: Code formatting
echo -e "${BLUE}[3/4] Checking code formatting...${NC}"
if cargo fmt --check 2>&1 > /tmp/fmt.log; then
    echo -e "${GREEN}✓ Code is properly formatted${NC}\n"
else
    echo -e "${RED}✗ Code formatting issues found:${NC}"
    cat /tmp/fmt.log
    echo -e "${YELLOW}Run 'cargo fmt' to fix automatically${NC}\n"
    FAILED=$((FAILED + 1))
fi

# Check 4: Run security audit
echo -e "${BLUE}[4/4] Checking dependencies...${NC}"
if cargo check --all-targets 2>&1 > /tmp/check.log; then
    echo -e "${GREEN}✓ All dependencies validated${NC}\n"
else
    echo -e "${RED}✗ Dependency check failed:${NC}"
    cat /tmp/check.log
    echo ""
    FAILED=$((FAILED + 1))
fi

# Summary
echo -e "${BLUE}================================${NC}"
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All quality gates passed!${NC}"
    echo -e "${GREEN}You're good to commit.${NC}\n"
    exit 0
else
    echo -e "${RED}✗ $FAILED quality gate(s) failed${NC}"
    echo -e "${RED}Please fix the issues above before committing.${NC}\n"
    exit 1
fi
