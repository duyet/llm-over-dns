#!/bin/bash

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}Installing git hooks...${NC}\n"

# Get the root directory of the git repository
GIT_DIR=$(git rev-parse --git-dir)
HOOKS_DIR="$GIT_DIR/hooks"

# Create hooks directory if it doesn't exist
mkdir -p "$HOOKS_DIR"

# Create pre-commit hook
cat > "$HOOKS_DIR/pre-commit" << 'EOF'
#!/bin/bash

# Run the pre-commit quality checks
scripts/pre-commit.sh
EOF

chmod +x "$HOOKS_DIR/pre-commit"

echo -e "${GREEN}✓ Pre-commit hook installed${NC}"
echo -e "  Location: $HOOKS_DIR/pre-commit\n"

# Create commit-msg hook (optional - for commit message validation)
cat > "$HOOKS_DIR/commit-msg" << 'EOF'
#!/bin/bash

# Get commit message
COMMIT_MSG=$(cat "$1")

# Check if commit message follows conventional commits
if ! echo "$COMMIT_MSG" | grep -qE "^(feat|fix|docs|style|refactor|perf|test|chore)(\(.+\))?!?: .+"; then
    echo "❌ Commit message does not follow conventional commits format"
    echo ""
    echo "Valid formats:"
    echo "  feat(scope): description"
    echo "  fix(scope): description"
    echo "  docs: description"
    echo "  style: description"
    echo "  refactor(scope): description"
    echo "  perf(scope): description"
    echo "  test(scope): description"
    echo "  chore(scope): description"
    echo ""
    echo "Examples:"
    echo "  feat(dns): add support for AAAA records"
    echo "  fix(llm): handle API timeout correctly"
    echo "  docs: update installation guide"
    exit 1
fi
EOF

chmod +x "$HOOKS_DIR/commit-msg"

echo -e "${GREEN}✓ Commit message hook installed${NC}"
echo -e "  Location: $HOOKS_DIR/commit-msg\n"

# Create post-merge hook (optional - for dependency updates)
cat > "$HOOKS_DIR/post-merge" << 'EOF'
#!/bin/bash

# Check if Cargo.lock changed
if git diff-tree -r --name-only HEAD@{1} HEAD | grep -q "^Cargo.lock$"; then
    echo "⚠️  Cargo.lock changed. Running 'cargo check' to update dependencies..."
    cargo check --quiet || exit 1
fi
EOF

chmod +x "$HOOKS_DIR/post-merge"

echo -e "${GREEN}✓ Post-merge hook installed${NC}"
echo -e "  Location: $HOOKS_DIR/post-merge\n"

echo -e "${BLUE}================================${NC}"
echo -e "${GREEN}Git hooks installation complete!${NC}"
echo -e "${BLUE}================================${NC}\n"

echo "Installed hooks:"
echo "  • pre-commit: Runs quality checks before commit"
echo "  • commit-msg: Validates conventional commit format"
echo "  • post-merge: Updates dependencies after merge\n"

echo -e "${YELLOW}To bypass hooks when needed, use:${NC}"
echo "  git commit --no-verify\n"
