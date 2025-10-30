#!/bin/bash
# Deploy LLM over DNS service
# Usage: ./scripts/deploy.sh [action]
# Actions: start, stop, restart, logs, status

set -e

ACTION="${1:-start}"

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "LLM over DNS Deployment Script"
echo "============================================"
echo ""

# Validate environment
validate_env() {
    if [ ! -f .env ]; then
        echo -e "${RED}Error: .env file not found${NC}"
        echo "Please create .env file with:"
        echo "  OPENROUTER_API_KEY=your_api_key_here"
        exit 1
    fi

    if ! grep -q "OPENROUTER_API_KEY=" .env; then
        echo -e "${RED}Error: OPENROUTER_API_KEY not set in .env${NC}"
        exit 1
    fi

    # Check if API key is placeholder
    API_KEY=$(grep "OPENROUTER_API_KEY=" .env | cut -d'=' -f2 | tr -d ' ')
    if [ "$API_KEY" == "your_api_key_here" ] || [ -z "$API_KEY" ]; then
        echo -e "${YELLOW}Warning: OPENROUTER_API_KEY appears to be not set${NC}"
        read -p "Continue? (y/n) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
}

# Build image
build_image() {
    echo -e "${YELLOW}Building Docker image...${NC}"
    docker-compose build
    echo -e "${GREEN}✓ Image built successfully${NC}"
}

# Start service
start_service() {
    validate_env

    if docker-compose ps | grep -q "llm-over-dns.*Up"; then
        echo -e "${YELLOW}Service is already running${NC}"
        return
    fi

    echo -e "${YELLOW}Starting LLM over DNS service...${NC}"
    docker-compose up -d

    # Wait for container to be healthy
    echo "Waiting for service to start..."
    MAX_ATTEMPTS=30
    ATTEMPT=0
    while [ $ATTEMPT -lt $MAX_ATTEMPTS ]; do
        if docker-compose exec -T llm-dns timeout 2 bash -c 'echo -e "\x00\x00\x00\x00\x00\x00\x00\x00" | nc -u localhost 53' &>/dev/null; then
            echo -e "${GREEN}✓ Service started and is healthy${NC}"
            break
        fi
        ATTEMPT=$((ATTEMPT + 1))
        if [ $ATTEMPT -eq $MAX_ATTEMPTS ]; then
            echo -e "${RED}✗ Service failed to start${NC}"
            echo "Logs:"
            docker-compose logs llm-dns
            exit 1
        fi
        sleep 1
    done
    echo ""
}

# Stop service
stop_service() {
    echo -e "${YELLOW}Stopping LLM over DNS service...${NC}"
    docker-compose down
    echo -e "${GREEN}✓ Service stopped${NC}"
}

# Restart service
restart_service() {
    echo -e "${YELLOW}Restarting LLM over DNS service...${NC}"
    docker-compose restart
    echo -e "${GREEN}✓ Service restarted${NC}"
}

# Show logs
show_logs() {
    echo -e "${YELLOW}Showing service logs (Ctrl+C to stop)...${NC}"
    docker-compose logs -f llm-dns
}

# Show status
show_status() {
    echo -e "${YELLOW}Service status:${NC}"
    echo ""
    docker-compose ps
    echo ""

    if docker-compose ps | grep -q "llm-over-dns.*Up"; then
        echo -e "${GREEN}Service is running${NC}"
        echo ""
        echo "DNS Port: 53 (UDP)"
        echo ""
        echo "Test commands:"
        echo "  dig @localhost 'hello.world' TXT"
        echo "  nslookup -type=TXT hello.world localhost"
        echo ""
    else
        echo -e "${RED}Service is not running${NC}"
        echo ""
        echo "Start with: docker-compose up -d"
    fi
}

# Execute based on action
case "$ACTION" in
    start)
        start_service
        show_status
        ;;
    stop)
        stop_service
        ;;
    restart)
        restart_service
        ;;
    logs)
        show_logs
        ;;
    status|ps)
        show_status
        ;;
    build)
        build_image
        ;;
    *)
        echo "Usage: $0 [action]"
        echo ""
        echo "Actions:"
        echo "  start     - Start the service"
        echo "  stop      - Stop the service"
        echo "  restart   - Restart the service"
        echo "  logs      - Show live logs"
        echo "  status    - Show service status"
        echo "  build     - Build Docker image"
        exit 1
        ;;
esac
