#!/usr/bin/env sh
# LLM-over-DNS one-liner installer
# Usage:
#   curl -sSL https://raw.githubusercontent.com/duyet/llm-over-dns/main/install.sh | sh
#   curl -sSL https://raw.githubusercontent.com/duyet/llm-over-dns/main/install.sh | ANYROUTER_API_KEY=sk-xxx sh
#   curl -sSL https://raw.githubusercontent.com/duyet/llm-over-dns/main/install.sh | sh -s -- --api-key sk-xxx --domain example.com
set -e

# ─── Colours ─────────────────────────────────────────────────────────────────
if [ -t 1 ]; then
  RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
  BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'
else
  RED=''; GREEN=''; YELLOW=''; BLUE=''; CYAN=''; BOLD=''; RESET=''
fi

info()    { printf "${BLUE}[info]${RESET}  %s\n" "$*"; }
success() { printf "${GREEN}[ok]${RESET}    %s\n" "$*"; }
warn()    { printf "${YELLOW}[warn]${RESET}  %s\n" "$*"; }
error()   { printf "${RED}[error]${RESET} %s\n" "$*" >&2; exit 1; }
step()    { printf "\n${BOLD}${CYAN}==> %s${RESET}\n" "$*"; }

# ─── Banner ──────────────────────────────────────────────────────────────────
printf "%b" "${CYAN}"
cat <<'EOF'
  _     _     __  __       ___                  ____  _   _ ____
 | |   | |   |  \/  |     / _ \__   _____ _ __|  _ \| \ | / ___|
 | |   | |   | |\/| |____| | | \ \ / / _ \ '__| | | |  \| \___ \
 | |___| |___| |  | |____| |_| |\ V /  __/ |  | |_| | |\  |___) |
 |_____|_____|_|  |_|     \___/  \_/ \___|_|  |____/|_| \_|____/

 LLM over DNS — answer any question via DNS TXT query
EOF
printf "%b\n" "${RESET}"

# ─── Defaults ────────────────────────────────────────────────────────────────
INSTALL_DIR="${INSTALL_DIR:-/opt/llm-over-dns}"
REPO_URL="${REPO_URL:-https://github.com/duyet/llm-over-dns}"
RAW_URL="${RAW_URL:-https://raw.githubusercontent.com/duyet/llm-over-dns/main}"
DNS_PORT="${DNS_PORT:-5353}"
ANYROUTER_API_KEY="${ANYROUTER_API_KEY:-}"
OPENROUTER_API_KEY="${OPENROUTER_API_KEY:-}"
MODEL="${OPENROUTER_MODEL:-nvidia/nemotron-nano-12b-v2-vl:free}"
CACHE_TTL="${CACHE_TTL_SEC:-300}"
RATE_LIMIT_RPS="${RATE_LIMIT_RPS:-5.0}"
RATE_LIMIT_BURST="${RATE_LIMIT_BURST:-10.0}"

# ─── Argument parsing ─────────────────────────────────────────────────────────
while [ $# -gt 0 ]; do
  case "$1" in
    --api-key)      ANYROUTER_API_KEY="$2"; shift 2 ;;
    --openrouter)   OPENROUTER_API_KEY="$2"; shift 2 ;;
    --model)        MODEL="$2"; shift 2 ;;
    --port)         DNS_PORT="$2"; shift 2 ;;
    --dir)          INSTALL_DIR="$2"; shift 2 ;;
    --uninstall)    UNINSTALL=1; shift ;;
    --help|-h)
      cat <<HELP
Usage: install.sh [OPTIONS]

Options:
  --api-key KEY     AnyRouter API key (or set \$ANYROUTER_API_KEY)
  --openrouter KEY  OpenRouter API key (or set \$OPENROUTER_API_KEY)
  --model MODEL     LLM model slug (default: nvidia/nemotron-nano-12b-v2-vl:free)
  --port PORT       DNS listen port inside container (default: 5353; host 53->PORT via iptables)
  --dir DIR         Install directory (default: /opt/llm-over-dns)
  --uninstall       Stop and remove the service
  --help            Show this help

Environment variables:
  ANYROUTER_API_KEY, OPENROUTER_API_KEY, OPENROUTER_MODEL,
  CACHE_TTL_SEC, RATE_LIMIT_RPS, RATE_LIMIT_BURST, INSTALL_DIR
HELP
      exit 0 ;;
    *) warn "Unknown argument: $1"; shift ;;
  esac
done

# ─── Uninstall path ──────────────────────────────────────────────────────────
if [ "${UNINSTALL:-0}" = "1" ]; then
  step "Uninstalling LLM-over-DNS"
  if [ -d "$INSTALL_DIR" ]; then
    cd "$INSTALL_DIR"
    $COMPOSE_CMD down --remove-orphans 2>/dev/null || true
  fi
  iptables -t nat -D PREROUTING -p udp --dport 53 -j REDIRECT --to-port 5353 2>/dev/null || true
  iptables -t nat -D OUTPUT     -p udp --dport 53 -j REDIRECT --to-port 5353 2>/dev/null || true
  rm -rf "$INSTALL_DIR"
  success "Uninstalled. DNS rules removed."
  exit 0
fi

# ─── Root check ───────────────────────────────────────────────────────────────
if [ "$(id -u)" != "0" ]; then
  error "This script must be run as root (sudo sh install.sh or run as root)."
fi

# ─── OS detection ─────────────────────────────────────────────────────────────
step "Detecting system"

OS="$(uname -s)"
ARCH="$(uname -m)"
info "OS: $OS  Arch: $ARCH"

case "$OS" in
  Linux) ;;
  *) error "Unsupported OS: $OS. Only Linux is supported." ;;
esac

# ─── Detect package manager ───────────────────────────────────────────────────
if command -v apt-get >/dev/null 2>&1; then
  PKG_MGR="apt"
elif command -v dnf >/dev/null 2>&1; then
  PKG_MGR="dnf"
elif command -v yum >/dev/null 2>&1; then
  PKG_MGR="yum"
elif command -v apk >/dev/null 2>&1; then
  PKG_MGR="apk"
elif command -v pacman >/dev/null 2>&1; then
  PKG_MGR="pacman"
else
  warn "No known package manager found — assuming dependencies are installed."
  PKG_MGR="none"
fi
info "Package manager: ${PKG_MGR}"

# ─── Install dependencies ─────────────────────────────────────────────────────
step "Installing dependencies"

install_pkg() {
  case "$PKG_MGR" in
    apt)    apt-get install -y -qq "$@" ;;
    dnf)    dnf install -y -q "$@" ;;
    yum)    yum install -y -q "$@" ;;
    apk)    apk add --no-cache "$@" ;;
    pacman) pacman -S --noconfirm --quiet "$@" ;;
    none)   warn "Skipping: $*" ;;
  esac
}

# curl / git
for cmd in curl git iptables; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    info "Installing $cmd..."
    install_pkg "$cmd"
  else
    success "$cmd already installed"
  fi
done

# ─── Detect / install container runtime ───────────────────────────────────────
step "Detecting container runtime"

COMPOSE_CMD=""

if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  RUNTIME="docker"
  info "Found Docker"
  if docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD="docker compose"
  elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD="docker-compose"
  fi
elif command -v podman >/dev/null 2>&1; then
  RUNTIME="podman"
  info "Found Podman"
  if command -v podman-compose >/dev/null 2>&1; then
    COMPOSE_CMD="podman-compose"
  elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD="docker-compose"
  fi
fi

if [ -z "$RUNTIME" ]; then
  step "Installing Docker (no container runtime found)"
  case "$PKG_MGR" in
    apt)
      apt-get update -qq
      install_pkg ca-certificates curl gnupg lsb-release
      install -m 0755 -d /etc/apt/keyrings
      curl -fsSL https://download.docker.com/linux/$(. /etc/os-release && echo "$ID")/gpg \
        | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
      echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
https://download.docker.com/linux/$(. /etc/os-release && echo "$ID") \
$(lsb_release -cs) stable" > /etc/apt/sources.list.d/docker.list
      apt-get update -qq
      install_pkg docker-ce docker-ce-cli containerd.io docker-compose-plugin
      systemctl enable --now docker
      ;;
    dnf|yum)
      install_pkg dnf-plugins-core 2>/dev/null || true
      dnf config-manager --add-repo https://download.docker.com/linux/centos/docker-ce.repo 2>/dev/null || true
      install_pkg docker-ce docker-ce-cli containerd.io docker-compose-plugin
      systemctl enable --now docker
      ;;
    *)
      error "Cannot auto-install Docker on this system. Please install Docker or Podman manually."
      ;;
  esac
  RUNTIME="docker"
  COMPOSE_CMD="docker compose"
  success "Docker installed"
fi

if [ -z "$COMPOSE_CMD" ]; then
  # Try to install docker-compose v2 plugin
  info "Installing docker-compose..."
  case "$PKG_MGR" in
    apt) install_pkg docker-compose-plugin 2>/dev/null && COMPOSE_CMD="docker compose" || true ;;
  esac
  if [ -z "$COMPOSE_CMD" ]; then
    COMPOSE_VER="v2.27.0"
    COMPOSE_BIN="/usr/local/bin/docker-compose"
    curl -fsSL "https://github.com/docker/compose/releases/download/${COMPOSE_VER}/docker-compose-linux-$(uname -m)" \
      -o "$COMPOSE_BIN"
    chmod +x "$COMPOSE_BIN"
    COMPOSE_CMD="docker-compose"
  fi
fi

success "Runtime: $RUNTIME  Compose: $COMPOSE_CMD"

# ─── Fix systemd-resolved (free port 53) ────────────────────────────────────
step "Freeing port 53"

if systemctl is-active --quiet systemd-resolved 2>/dev/null; then
  RESOLVED_CONF="/etc/systemd/resolved.conf"
  if ! grep -q "^DNSStubListener=no" "$RESOLVED_CONF" 2>/dev/null; then
    # Ensure [Resolve] section exists
    if ! grep -q "^\[Resolve\]" "$RESOLVED_CONF" 2>/dev/null; then
      printf "\n[Resolve]\n" >> "$RESOLVED_CONF"
    fi
    printf "DNSStubListener=no\n" >> "$RESOLVED_CONF"
    systemctl restart systemd-resolved
    info "Disabled systemd-resolved DNSStubListener"
  else
    info "systemd-resolved stub already disabled"
  fi
fi

# ─── Clone / update repo ──────────────────────────────────────────────────────
step "Setting up files in ${INSTALL_DIR}"

mkdir -p "$INSTALL_DIR"

if [ -d "${INSTALL_DIR}/.git" ]; then
  info "Updating existing clone..."
  git -C "$INSTALL_DIR" pull --ff-only
else
  info "Cloning repository..."
  git clone "$REPO_URL" "$INSTALL_DIR"
fi

cd "$INSTALL_DIR"

# ─── Write .env ───────────────────────────────────────────────────────────────
step "Writing .env"

# Prompt for API key if not provided
if [ -z "$ANYROUTER_API_KEY" ] && [ -z "$OPENROUTER_API_KEY" ]; then
  if [ -t 0 ]; then
    printf "${YELLOW}Enter your AnyRouter or OpenRouter API key: ${RESET}"
    read -r INPUT_KEY
    if echo "$INPUT_KEY" | grep -q "^sk-ant\|anyrouter"; then
      ANYROUTER_API_KEY="$INPUT_KEY"
    else
      OPENROUTER_API_KEY="$INPUT_KEY"
    fi
  else
    warn "No API key provided. Set ANYROUTER_API_KEY or OPENROUTER_API_KEY before starting."
  fi
fi

cat > "${INSTALL_DIR}/.env" <<ENV
# Generated by install.sh on $(date -u +%Y-%m-%dT%H:%M:%SZ)
ANYROUTER_API_KEY=${ANYROUTER_API_KEY}
OPENROUTER_API_KEY=${OPENROUTER_API_KEY}
OPENROUTER_MODEL=${MODEL}
DNS_ADDRESS=0.0.0.0
DNS_PORT=${DNS_PORT}
CACHE_TTL_SEC=${CACHE_TTL}
RATE_LIMIT_RPS=${RATE_LIMIT_RPS}
RATE_LIMIT_BURST=${RATE_LIMIT_BURST}
RUST_LOG=info
ENV
chmod 600 "${INSTALL_DIR}/.env"
success ".env written"

# ─── Build + start ────────────────────────────────────────────────────────────
step "Building and starting container (this takes ~5-8 min on first run)"

$COMPOSE_CMD build
$COMPOSE_CMD up -d

# ─── iptables: redirect UDP 53 → DNS_PORT ─────────────────────────────────────
step "Setting up iptables port redirect 53 → ${DNS_PORT}"

# Idempotent: only add if not already present
iptables -t nat -C PREROUTING -p udp --dport 53 -j REDIRECT --to-port "$DNS_PORT" 2>/dev/null || \
  iptables -t nat -A PREROUTING -p udp --dport 53 -j REDIRECT --to-port "$DNS_PORT"

# Persist iptables rules across reboots
if command -v netfilter-persistent >/dev/null 2>&1; then
  netfilter-persistent save
elif command -v iptables-save >/dev/null 2>&1; then
  RULES_FILE=""
  if [ -d /etc/iptables ]; then
    RULES_FILE="/etc/iptables/rules.v4"
  elif [ -d /etc/sysconfig ]; then
    RULES_FILE="/etc/sysconfig/iptables"
  fi
  if [ -n "$RULES_FILE" ]; then
    iptables-save > "$RULES_FILE"
    info "iptables rules saved to $RULES_FILE"
  fi
fi

# Add to /etc/rc.local as fallback for persistence
RC_LOCAL="/etc/rc.local"
IPTR="iptables -t nat -C PREROUTING -p udp --dport 53 -j REDIRECT --to-port ${DNS_PORT} 2>/dev/null || iptables -t nat -A PREROUTING -p udp --dport 53 -j REDIRECT --to-port ${DNS_PORT}"
if [ -f "$RC_LOCAL" ]; then
  if ! grep -q "llm-over-dns" "$RC_LOCAL"; then
    sed -i "s|^exit 0|# llm-over-dns\n${IPTR}\nexit 0|" "$RC_LOCAL" 2>/dev/null || \
      echo "$IPTR" >> "$RC_LOCAL"
    info "iptables rule added to $RC_LOCAL"
  fi
fi

success "iptables redirect: UDP :53 → :${DNS_PORT}"

# ─── Verify ───────────────────────────────────────────────────────────────────
step "Verifying deployment"

sleep 3
STATUS=$($COMPOSE_CMD ps --format "{{.Status}}" 2>/dev/null | head -1 || \
         $COMPOSE_CMD ps 2>/dev/null | grep llm-over-dns | awk '{print $4}')

if echo "$STATUS" | grep -qi "up\|running"; then
  success "Container is UP"
else
  warn "Container status: ${STATUS}"
  info "Check logs: cd ${INSTALL_DIR} && ${COMPOSE_CMD} logs -f"
fi

# Quick DNS test
SERVER_IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
printf "\n${BOLD}Testing DNS (may take a moment for first query):${RESET}\n"
if command -v dig >/dev/null 2>&1; then
  dig +short +time=5 TXT "what.is.2+2" "@127.0.0.1" 2>/dev/null | head -3 || \
    warn "DNS test timed out (normal on first cold start — LLM call in progress)"
elif command -v nslookup >/dev/null 2>&1; then
  nslookup -type=TXT "what.is.2+2" 127.0.0.1 2>/dev/null | grep -i '"' | head -3 || \
    warn "DNS test timed out (normal on first cold start)"
fi

# ─── Done ─────────────────────────────────────────────────────────────────────
printf "\n"
printf "%b" "${GREEN}${BOLD}"
cat <<EOF
╔══════════════════════════════════════════════════════════════╗
║  LLM-over-DNS is live!                                       ║
║                                                              ║
║  Ask anything via DNS:                                       ║
║    dig +short TXT "what.is.the.capital.of.france" @${SERVER_IP:-YOUR_IP}   ║
║    dig +short TXT "explain.quantum.entanglement" @${SERVER_IP:-YOUR_IP}    ║
║                                                              ║
║  Manage:                                                     ║
║    cd ${INSTALL_DIR}                               ║
║    ${COMPOSE_CMD} logs -f          # live logs               ║
║    ${COMPOSE_CMD} restart          # restart                 ║
║    ${COMPOSE_CMD} down             # stop                    ║
║                                                              ║
║  Uninstall:                                                  ║
║    curl -sSL ${RAW_URL}/install.sh | sh -s -- --uninstall   ║
╚══════════════════════════════════════════════════════════════╝
EOF
printf "%b\n" "${RESET}"
