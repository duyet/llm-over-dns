#!/bin/bash
# Test DNS server functionality
# Usage: ./scripts/test.sh [hostname] [port]

set -e

HOSTNAME="${1:-hello.world}"
PORT="${2:-53}"
LOCALHOST="127.0.0.1"

echo "Testing DNS server on ${LOCALHOST}:${PORT}"
echo "============================================"
echo ""

# Function to test DNS query with different tools
test_dns() {
    local tool=$1
    local cmd=$2

    echo "Testing with: $tool"
    if command -v ${tool} &> /dev/null; then
        eval "$cmd" && echo "✓ ${tool} test passed" || echo "✗ ${tool} test failed"
    else
        echo "⊘ ${tool} not found, skipping"
    fi
    echo ""
}

# Wait for container to be healthy
echo "Waiting for DNS server to be ready..."
MAX_ATTEMPTS=30
ATTEMPT=0
while [ $ATTEMPT -lt $MAX_ATTEMPTS ]; do
    if docker-compose exec llm-dns timeout 2 bash -c 'echo -e "\x00\x00\x00\x00\x00\x00\x00\x00" | nc -u localhost 53' &>/dev/null; then
        echo "✓ DNS server is ready"
        break
    fi
    ATTEMPT=$((ATTEMPT + 1))
    if [ $ATTEMPT -eq $MAX_ATTEMPTS ]; then
        echo "✗ DNS server failed to start"
        docker-compose logs llm-dns
        exit 1
    fi
    sleep 1
done
echo ""

# Test 1: dig
test_dns "dig" "dig @${LOCALHOST} -p ${PORT} ${HOSTNAME} TXT +short"

# Test 2: nslookup
test_dns "nslookup" "nslookup -type=TXT ${HOSTNAME} ${LOCALHOST}:${PORT}"

# Test 3: host
test_dns "host" "host -t TXT ${HOSTNAME} ${LOCALHOST}:${PORT}"

# Test 4: nc (raw DNS query)
if command -v nc &> /dev/null; then
    echo "Testing with: nc (raw DNS)"
    # Create a simple DNS query for the hostname
    # This is a very basic query, just to verify port is listening
    if timeout 2 bash -c "echo -e '\x00\x00\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00' | nc -u ${LOCALHOST} ${PORT}" &>/dev/null; then
        echo "✓ nc test passed"
    else
        echo "✗ nc test failed"
    fi
    echo ""
fi

# Get container logs for reference
echo "Recent container logs:"
echo "============================================"
docker-compose logs --tail=20 llm-dns || echo "No logs available"

echo ""
echo "Test complete!"
echo ""
echo "Useful debugging commands:"
echo "  docker-compose logs -f llm-dns          # Live logs"
echo "  docker-compose ps                       # Container status"
echo "  docker-compose exec llm-dns env         # Environment variables"
