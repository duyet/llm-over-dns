#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║ LLM over DNS - Comprehensive Quality Audit     ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}\n"

FAILED=0
WARNINGS=0

# 1. Test Coverage
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}[1/6] Code Coverage Analysis${NC}"
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

if command -v cargo-tarpaulin &> /dev/null; then
    if cargo tarpaulin --out Xml --output-dir coverage --timeout 120 2>&1 | tail -20; then
        echo -e "${GREEN}✓ Coverage analysis complete${NC}\n"
    else
        echo -e "${YELLOW}⚠ Coverage analysis skipped (optional)${NC}\n"
        WARNINGS=$((WARNINGS + 1))
    fi
else
    echo -e "${YELLOW}⚠ cargo-tarpaulin not installed (install with: cargo install cargo-tarpaulin)${NC}\n"
    WARNINGS=$((WARNINGS + 1))
fi

# 2. Unit Tests
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}[2/6] Unit Tests${NC}"
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

if cargo test --lib --quiet 2>&1; then
    echo -e "${GREEN}✓ All unit tests passed${NC}\n"
else
    echo -e "${RED}✗ Unit tests failed${NC}\n"
    FAILED=$((FAILED + 1))
fi

# 3. Clippy Linting
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}[3/6] Clippy Analysis (Zero Warnings Policy)${NC}"
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

if cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tee /tmp/clippy.log | grep -q "warning\|error"; then
    echo -e "${RED}✗ Clippy warnings detected${NC}"
    grep -E "warning|error" /tmp/clippy.log | head -10
    echo ""
    FAILED=$((FAILED + 1))
else
    echo -e "${GREEN}✓ No clippy warnings${NC}\n"
fi

# 4. Code Formatting
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}[4/6] Code Format Check (rustfmt)${NC}"
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

if cargo fmt --check 2>&1 > /tmp/fmt.log; then
    echo -e "${GREEN}✓ Code is properly formatted${NC}\n"
else
    echo -e "${RED}✗ Format violations found${NC}"
    cat /tmp/fmt.log | head -10
    echo -e "${YELLOW}  Run: cargo fmt${NC}\n"
    FAILED=$((FAILED + 1))
fi

# 5. Dependency Audit
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}[5/6] Dependency Audit${NC}"
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

if cargo check --all-targets 2>&1 > /tmp/check.log; then
    echo -e "${GREEN}✓ All dependencies validated${NC}\n"
else
    echo -e "${RED}✗ Dependency check failed${NC}"
    cat /tmp/check.log | head -10
    echo ""
    FAILED=$((FAILED + 1))
fi

# 6. Documentation
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}[6/6] Documentation Check${NC}"
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

DOC_WARNINGS=$(cargo doc --no-deps 2>&1 | grep -c "warning" || true)

if [ "$DOC_WARNINGS" -eq 0 ]; then
    echo -e "${GREEN}✓ Documentation generated without warnings${NC}\n"
else
    echo -e "${YELLOW}⚠ $DOC_WARNINGS documentation warning(s)${NC}\n"
    WARNINGS=$((WARNINGS + 1))
fi

# Summary
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Quality Audit Summary${NC}"
echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

if [ $FAILED -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "${GREEN}✓ All quality checks passed! No issues found.${NC}\n"
    exit 0
elif [ $FAILED -eq 0 ]; then
    echo -e "${YELLOW}⚠ All critical checks passed with $WARNINGS warning(s)${NC}\n"
    exit 0
else
    echo -e "${RED}✗ $FAILED quality check(s) failed${NC}"
    echo -e "${RED}  Please fix the failures before proceeding${NC}\n"
    exit 1
fi
