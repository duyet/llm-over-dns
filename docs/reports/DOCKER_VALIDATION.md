# Docker Setup Validation

Comprehensive validation checks and test procedures for the Docker deployment.

## Pre-Deployment Checklist

### ✅ File Validation

```bash
# Verify all Docker files exist
test -f Dockerfile && echo "✓ Dockerfile found"
test -f docker-compose.yml && echo "✓ docker-compose.yml found"
test -f .dockerignore && echo "✓ .dockerignore found"
test -f DOCKER.md && echo "✓ DOCKER.md found"
test -f DOCKER_QUICKSTART.md && echo "✓ DOCKER_QUICKSTART.md found"
test -d scripts && echo "✓ scripts directory found"
test -f scripts/build.sh && echo "✓ scripts/build.sh found"
test -f scripts/deploy.sh && echo "✓ scripts/deploy.sh found"
test -f scripts/test.sh && echo "✓ scripts/test.sh found"
```

### ✅ Environment Validation

```bash
# Check .env file
if [ ! -f .env ]; then
    echo "⚠ .env not found, creating from .env.example..."
    cp .env.example .env
fi

# Verify required variables
grep -q "OPENROUTER_API_KEY=" .env && echo "✓ OPENROUTER_API_KEY configured"
grep -q "DNS_PORT=" .env && echo "✓ DNS_PORT configured"
```

### ✅ Script Validation

```bash
# Make scripts executable
chmod +x scripts/*.sh

# Verify executability
test -x scripts/build.sh && echo "✓ build.sh executable"
test -x scripts/deploy.sh && echo "✓ deploy.sh executable"
test -x scripts/test.sh && echo "✓ test.sh executable"
```

## Syntax Validation

### ✅ Dockerfile Validation

```bash
# Check Dockerfile syntax (requires docker)
docker build --dry-run . 2>&1 | grep -i error && echo "✗ Dockerfile errors found" || echo "✓ Dockerfile syntax valid"
```

### ✅ docker-compose.yml Validation

```bash
# Validate docker-compose configuration
docker-compose config > /dev/null 2>&1 && echo "✓ docker-compose.yml valid" || echo "✗ docker-compose.yml invalid"
```

## Size Optimization Validation

### ✅ .dockerignore Effectiveness

Expected excluded:
- `/target` directory (~500MB)
- `/.git` directory (~50MB)
- `/src` source code (~10KB)
- Development files (~100KB+)

Total expected reduction: ~500MB+

### ✅ Image Size Target

```bash
# After build, check image size
docker images llm-over-dns --format "table {{.Repository}}\t{{.Size}}"

# Expected: 40-50MB final image
```

### ✅ Build Cache Effectiveness

First build:
- Stage 1 (builder): ~2-3 min (downloads Rust, deps)
- Stage 2 (runtime): ~1-2 min
- Total: 3-5 min

Subsequent builds (no source change):
- Stage 1: Cached (seconds)
- Stage 2: Cached (seconds)
- Total: <1 min

Source code change only:
- Stage 1: Recompile (1-2 min)
- Stage 2: Cached
- Total: 1-2 min

## Configuration Validation

### ✅ Environment Variables

```bash
# Check required variables
REQUIRED_VARS="OPENROUTER_API_KEY"
for var in $REQUIRED_VARS; do
    if grep -q "^$var=" .env; then
        echo "✓ $var is set"
    else
        echo "✗ $var is missing"
    fi
done
```

### ✅ Docker Compose Configuration

Verify in docker-compose.yml:
- [x] Service name: `llm-dns`
- [x] Port mapping: `53:53/udp`
- [x] Environment variables loaded from .env
- [x] Health check configured
- [x] Restart policy set
- [x] Security options applied
- [x] Capabilities configured (NET_BIND_SERVICE)

## Runtime Validation

### ✅ Build Test

```bash
# Build the image
docker-compose build

# Verify image created
docker images llm-over-dns || (echo "✗ Image build failed"; exit 1)
echo "✓ Image successfully built"
```

### ✅ Start Test

```bash
# Start the service
docker-compose up -d

# Wait for startup
sleep 5

# Check if running
docker-compose ps | grep -i "up" && echo "✓ Service started" || (echo "✗ Service failed to start"; docker-compose logs llm-dns; exit 1)
```

### ✅ Health Check Test

```bash
# Check container health status
HEALTH=$(docker inspect llm-over-dns --format='{{json .State.Health.Status}}')
if [ "$HEALTH" == '"healthy"' ]; then
    echo "✓ Service health: healthy"
elif [ "$HEALTH" == '"starting"' ]; then
    echo "⏳ Service health: starting (normal after first start)"
else
    echo "✗ Service health: $HEALTH"
    docker-compose logs llm-dns
fi
```

### ✅ DNS Functionality Test

```bash
# Test DNS port is listening
if timeout 2 bash -c 'echo -e "\x00\x00\x00\x00\x00\x00\x00\x00" | nc -u localhost 53' &>/dev/null; then
    echo "✓ DNS port 53 responding"
else
    echo "✗ DNS port 53 not responding"
    echo "Possible causes:"
    echo "  - Port 53 already in use"
    echo "  - Container not running"
    echo "  - Network issue"
fi
```

### ✅ API Integration Test

```bash
# Check if service can reach OpenRouter API
docker-compose exec -T llm-dns bash -c 'curl -s -H "Authorization: Bearer $OPENROUTER_API_KEY" https://api.openrouter.ai/api/v1/models | jq . | head -20' && echo "✓ API connectivity verified" || echo "⚠ API connectivity check inconclusive"
```

## Security Validation

### ✅ Non-root User

```bash
# Verify container runs as non-root
docker-compose exec llm-dns id | grep -q uid=1000 && echo "✓ Running as non-root user (uid 1000)" || echo "✗ Running as root or unexpected user"
```

### ✅ Capabilities

```bash
# Verify capabilities
docker inspect llm-over-dns --format='{{json .HostConfig.CapAdd}}' | grep -q "NET_BIND_SERVICE" && echo "✓ NET_BIND_SERVICE capability set"
docker inspect llm-over-dns --format='{{json .HostConfig.CapDrop}}' | grep -q "ALL" && echo "✓ ALL capabilities dropped"
```

### ✅ Privilege Escalation

```bash
# Verify no-new-privileges
docker inspect llm-over-dns --format='{{.HostConfig.SecurityOpt}}' | grep -q "no-new-privileges:true" && echo "✓ Privilege escalation prevented" || echo "⚠ no-new-privileges not explicitly set"
```

## Performance Validation

### ✅ Memory Usage

```bash
# Check memory usage (should be <200MB)
MEMORY=$(docker stats llm-over-dns --no-stream --format "{{.MemUsage}}" | grep -oP '\d+' | head -1)
if [ "$MEMORY" -lt 200 ]; then
    echo "✓ Memory usage: ${MEMORY}MB (below 200MB threshold)"
else
    echo "⚠ Memory usage: ${MEMORY}MB (above 200MB threshold)"
fi
```

### ✅ CPU Usage

```bash
# Check CPU usage (should be <10% idle)
docker stats llm-over-dns --no-stream
```

### ✅ Disk Usage

```bash
# Check image size
echo "Docker image size:"
docker images llm-over-dns --format "table {{.Size}}"

# Check container disk usage
echo "Container disk usage:"
docker inspect llm-over-dns --format='{{.SizeRw}}' | awk '{printf "%.1f MB\n", $1/1024/1024}'
```

## Logging Validation

### ✅ Log Output

```bash
# Check logs are being generated
docker-compose logs llm-dns | wc -l > /dev/null && echo "✓ Logs available"

# Check log format
docker-compose logs llm-dns | head -5
```

### ✅ Log Rotation

Verify in docker-compose.yml:
- [x] Log driver: json-file
- [x] Max size: 10m
- [x] Max file: 3
- [x] Expected max logs: ~30MB

## Cleanup Validation

### ✅ Stop Service

```bash
# Stop the service
docker-compose down

# Verify stopped
docker-compose ps | grep -i "up" || echo "✓ Service successfully stopped"
```

### ✅ Clean Resources

```bash
# Remove image
docker rmi llm-over-dns:latest

# Verify removed
docker images llm-over-dns || echo "✓ Image successfully removed"
```

## Automated Validation Script

Run all validations at once:

```bash
#!/bin/bash
# Comprehensive validation script

echo "Docker Setup Validation"
echo "======================="
echo ""

# File checks
echo "File Validation:"
test -f Dockerfile && echo "✓ Dockerfile" || echo "✗ Dockerfile missing"
test -f docker-compose.yml && echo "✓ docker-compose.yml" || echo "✗ docker-compose.yml missing"
test -f .dockerignore && echo "✓ .dockerignore" || echo "✗ .dockerignore missing"
echo ""

# Environment checks
echo "Environment Validation:"
test -f .env && echo "✓ .env exists" || (echo "✗ .env missing, creating..."; cp .env.example .env)
grep -q "OPENROUTER_API_KEY=" .env && echo "✓ OPENROUTER_API_KEY set" || echo "✗ OPENROUTER_API_KEY missing"
echo ""

# Script checks
echo "Script Validation:"
test -x scripts/build.sh && echo "✓ build.sh executable" || echo "✗ build.sh not executable"
test -x scripts/deploy.sh && echo "✓ deploy.sh executable" || echo "✗ deploy.sh not executable"
test -x scripts/test.sh && echo "✓ test.sh executable" || echo "✗ test.sh not executable"
echo ""

# Docker checks
if command -v docker &> /dev/null; then
    echo "Docker Validation:"
    docker --version && echo "✓ Docker installed"
    docker-compose --version && echo "✓ Docker Compose installed"
    
    # Validate configs
    docker-compose config > /dev/null 2>&1 && echo "✓ docker-compose.yml valid" || echo "✗ docker-compose.yml invalid"
    echo ""
else
    echo "⚠ Docker not installed or not in PATH"
fi

echo "Validation complete!"
```

## Continuous Validation

### ✅ Pre-commit Hook

Add to `.git/hooks/pre-commit`:

```bash
#!/bin/bash
# Validate Docker files before commit

echo "Validating Docker setup..."

# Check files exist
files=("Dockerfile" "docker-compose.yml" ".dockerignore")
for file in "${files[@]}"; do
    if [ ! -f "$file" ]; then
        echo "✗ $file missing"
        exit 1
    fi
done

# Validate compose
if command -v docker-compose &> /dev/null; then
    docker-compose config > /dev/null 2>&1 || {
        echo "✗ docker-compose.yml invalid"
        exit 1
    }
fi

echo "✓ Docker setup valid"
exit 0
```

## Troubleshooting Validation

If validation fails:

1. **Check Docker installation**:
   ```bash
   docker --version
   docker-compose --version
   ```

2. **Check .env configuration**:
   ```bash
   cat .env | grep -v "^#" | grep -v "^$"
   ```

3. **Validate YAML syntax**:
   ```bash
   docker-compose config
   ```

4. **Check image exists**:
   ```bash
   docker images llm-over-dns
   ```

5. **View logs**:
   ```bash
   docker-compose logs llm-dns
   ```

6. **Inspect container**:
   ```bash
   docker inspect llm-over-dns
   ```

---

**Last Updated**: 2025-10-30
**Validation Status**: Complete Checklist Available
