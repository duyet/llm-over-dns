#!/bin/bash
# Build Docker image for LLM over DNS
# Usage: ./scripts/build.sh [tag]

set -e

TAG="${1:-latest}"
IMAGE_NAME="llm-over-dns:${TAG}"

echo "Building Docker image: $IMAGE_NAME"
echo "============================================"

# Check if .env file exists
if [ ! -f .env ]; then
    echo "Warning: .env file not found"
    echo "Creating .env from .env.example..."
    cp .env.example .env
    echo "Please update .env with your OpenRouter API key"
fi

# Build using docker-compose
docker-compose build

echo ""
echo "Build complete!"
echo "Image: $IMAGE_NAME"
echo ""
echo "Next steps:"
echo "  1. Update .env with your OPENROUTER_API_KEY"
echo "  2. Run: docker-compose up -d"
echo "  3. Test: dig @localhost 'hello.world' TXT"
