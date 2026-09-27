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
# Empty by default. Hard-coding a model here would override the application's own
# multi-model fallback list, which is a headline feature — so only an explicit
# --model (or OPENROUTER_MODEL) is written to .env, and otherwise the app default
# applies. An empty value is NOT equivalent to unset: it yields an empty model
# list, which aborts startup.
MODEL="${OPENROUTER_MODEL:-}"
CACHE_TTL="${CACHE_TTL_SEC:-300}"
RATE_LIMIT_RPS="${RATE_LIMIT_RPS:-5.0}"
RATE_LIMIT_BURST="${RATE_LIMIT_BURST:-10.0}"
# Rewritten by the install path and restored by the uninstall path.
RESOLVED_CONF="/etc/systemd/resolved.conf"

# Resolve the container runtime and its compose command. Shared with the
# uninstall path, which runs before the install-time detection below and must
# never reach `$COMPOSE_CMD down` with an unset command.
detect_runtime() {
  RUNTIME=""
  COMPOSE_CMD=""
  if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    RUNTIME="docker"
    if docker compose version >/dev/null 2>&1; then
      COMPOSE_CMD="docker compose"
    elif command -v docker-compose >/dev/null 2>&1; then
      COMPOSE_CMD="docker-compose"
    fi
  elif command -v podman >/dev/null 2>&1; then
    RUNTIME="podman"
    if command -v podman-compose >/dev/null 2>&1; then
      COMPOSE_CMD="podman-compose"
    elif command -v docker-compose >/dev/null 2>&1; then
      COMPOSE_CMD="docker-compose"
    fi
  fi
}

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
  --model MODEL     LLM model slug, for the selected provider (default: the
                    application's multi-model fallback list)
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

  # `rm -rf` below is unguarded, and --dir reaches it: refuse an empty or root
  # install directory rather than taking the filesystem with it.
  case "$INSTALL_DIR" in
    ""|"/") error "Refusing to remove install directory '${INSTALL_DIR}' — pass --dir with a real path." ;;
  esac

  # Resolve the compose command first. This branch used to run before the
  # install-time detection assigned it, so "down" expanded to a command with no
  # program name, failed, and was suppressed — leaving the container running
  # while the compose file that defines it was deleted.
  detect_runtime
  if [ -n "$COMPOSE_CMD" ] && [ -d "$INSTALL_DIR" ]; then
    # Stop the container before its compose file disappears. Subshell so the
    # later rm -rf still resolves a relative --dir against the original cwd.
    ( cd "$INSTALL_DIR" && $COMPOSE_CMD down --remove-orphans ) 2>/dev/null || \
      warn "Could not stop the container via '${COMPOSE_CMD}'. Stop it manually: docker stop llm-over-dns"
  elif [ -z "$COMPOSE_CMD" ]; then
    warn "No container runtime found — the container may still be running. Stop it manually: docker stop llm-over-dns"
  fi

  # Remove both chains the install path creates, for the default port and for a
  # custom one (--port).
  for port in 5353 "$DNS_PORT"; do
    iptables -t nat -D PREROUTING -p udp --dport 53 -j REDIRECT --to-port "$port" 2>/dev/null || true
    iptables -t nat -D OUTPUT     -p udp --dport 53 -j REDIRECT --to-port "$port" 2>/dev/null || true
  done

  # Hand :53 back to systemd-resolved, which the install path took over.
  if [ -f "$RESOLVED_CONF" ] && grep -q "^DNSStubListener=no$" "$RESOLVED_CONF" 2>/dev/null; then
    sed -i '/^DNSStubListener=no$/d' "$RESOLVED_CONF" 2>/dev/null || \
      warn "Could not restore DNSStubListener in ${RESOLVED_CONF}."
    systemctl restart systemd-resolved 2>/dev/null || \
      warn "Could not restart systemd-resolved."
    info "Restored systemd-resolved DNSStubListener"
  fi

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

detect_runtime
if [ -n "$RUNTIME" ]; then
  info "Found $RUNTIME"
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

# Write only the variables that are actually set. An empty stub is not
# equivalent to an absent one: the application treats the *presence* of
# ANYROUTER_API_KEY as intent to use AnyRouter (so an empty key alongside an
# OpenRouter key yields 401s on every call), and an empty model list aborts
# startup. Leaving a variable out lets the application default apply.
ENV_FILE="${INSTALL_DIR}/.env"
printf '# Generated by install.sh on %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$ENV_FILE"

if [ -n "$ANYROUTER_API_KEY" ]; then
  printf 'ANYROUTER_API_KEY=%s\n' "$ANYROUTER_API_KEY" >> "$ENV_FILE"
fi
if [ -n "$OPENROUTER_API_KEY" ]; then
  printf 'OPENROUTER_API_KEY=%s\n' "$OPENROUTER_API_KEY" >> "$ENV_FILE"
fi
# The model list belongs to the active provider: the application reads
# ANYROUTER_MODEL for AnyRouter and OPENROUTER_MODEL otherwise, and AnyRouter
# takes precedence when both keys are present.
if [ -n "$MODEL" ]; then
  if [ -n "$ANYROUTER_API_KEY" ]; then
    printf 'ANYROUTER_MODEL=%s\n' "$MODEL" >> "$ENV_FILE"
  else
    printf 'OPENROUTER_MODEL=%s\n' "$MODEL" >> "$ENV_FILE"
  fi
fi

cat >> "$ENV_FILE" <<ENV
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

# Locally-originated packets traverse OUTPUT, not PREROUTING, so the matching
# OUTPUT rule is what makes the host's own resolver — and the verification query
# below — reach the service. Guarded the same way as PREROUTING.
iptables -t nat -C OUTPUT -p udp --dport 53 -j REDIRECT --to-port "$DNS_PORT" 2>/dev/null || \
  iptables -t nat -A OUTPUT -p udp --dport 53 -j REDIRECT --to-port "$DNS_PORT"

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
# Both chains on one line, and the sed below uses @ as its delimiter: the
# `||` in the rules would otherwise close the s command early, so the edit
# failed and the rules were appended after `exit 0`, where rc.local skips them.
IPTR="iptables -t nat -C PREROUTING -p udp --dport 53 -j REDIRECT --to-port ${DNS_PORT} 2>/dev/null || iptables -t nat -A PREROUTING -p udp --dport 53 -j REDIRECT --to-port ${DNS_PORT}; iptables -t nat -C OUTPUT -p udp --dport 53 -j REDIRECT --to-port ${DNS_PORT} 2>/dev/null || iptables -t nat -A OUTPUT -p udp --dport 53 -j REDIRECT --to-port ${DNS_PORT}"
if [ -f "$RC_LOCAL" ]; then
  if ! grep -q "llm-over-dns" "$RC_LOCAL"; then
    sed -i "s@^exit 0@# llm-over-dns\n${IPTR}\nexit 0@" "$RC_LOCAL" 2>/dev/null || \
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

# Quick DNS test. The answer is a real LLM call, so allow a cold start.
# The exit status of a pipeline comes from its last command, so the old
# `... | head -3 || warn` could never fail: `head` always succeeds and the
# success banner was printed regardless. Capture the answer and decide on that.
SERVER_IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
printf "\n${BOLD}Testing DNS (may take a moment for first query):${RESET}\n"

if command -v dig >/dev/null 2>&1; then
  DNS_TESTED="yes"
  # dig and nslookup report "connection refused" / "no servers could be
  # reached" on STDOUT and exit non-zero, so judging the output alone would call
  # a dead resolver healthy. The exit status is the verdict; keep just the
  # answer records. The assignment sits in an `if` so `set -e` stands down.
  if DNS_OUT="$(dig +short +time=15 TXT "what.is.2+2" "@127.0.0.1" 2>/dev/null)"; then
    DNS_ANSWER="$(printf '%s\n' "$DNS_OUT" | grep -v '^;;')"
  else
    DNS_ANSWER=""
  fi
elif command -v nslookup >/dev/null 2>&1; then
  DNS_TESTED="yes"
  if DNS_OUT="$(nslookup -type=TXT "what.is.2+2" 127.0.0.1 2>/dev/null)"; then
    DNS_ANSWER="$(printf '%s\n' "$DNS_OUT" | grep -i '"')"
  else
    DNS_ANSWER=""
  fi
else
  DNS_TESTED="no"
  DNS_ANSWER=""
  warn "Neither dig nor nslookup is installed — skipping the DNS query test."
fi

if [ "$DNS_TESTED" = "yes" ]; then
  if [ -n "$DNS_ANSWER" ]; then
    printf '%s\n' "$DNS_ANSWER" | head -3
    success "DNS query answered"
  else
    # Fail loudly instead of printing a success banner over a dead service.
    error "No TXT answer from 127.0.0.1:53 — the DNS service is not responding.
  Inspect it with: cd ${INSTALL_DIR} && ${COMPOSE_CMD} logs -f"
  fi
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
