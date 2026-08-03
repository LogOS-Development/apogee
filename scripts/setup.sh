#!/usr/bin/env bash
# Apogee unified setup script
#
# Sets up both the Rust workspace and the Python helper environment.
# Idempotent: safe to re-run.
#
# Usage:
#   ./scripts/setup.sh              # full setup (run in subshell)
#   ./scripts/setup.sh --skip-data  # skip ./scripts/fetch_data.sh
#   ./scripts/setup.sh --minimal    # skip data fetch and coverage tools
#   source ./scripts/setup.sh       # setup + activate venv in current shell
#
set -euo pipefail

# Detect if the script is being sourced. If so, after completing setup we
# will activate the uv-managed virtualenv in the caller's shell.
SOURCED=false
if [[ "${BASH_SOURCE[0]}" != "${0}" ]]; then
    SOURCED=true
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
VENV_DIR="$ROOT_DIR/.venv"

SKIP_DATA=false
MINIMAL=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-data) SKIP_DATA=true ;;
        --minimal)   SKIP_DATA=true; MINIMAL=true ;;
        -h|--help)
            sed -n '2,15p' "$0"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
    shift
done

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log()   { echo -e "${GREEN}[SETUP]${NC} $1"; }
info()  { echo -e "${BLUE}[INFO]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
fail()  { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

command_exists() { command -v "$1" >/dev/null 2>&1; }

log "Apogee setup starting in $ROOT_DIR"
cd "$ROOT_DIR"

# ─────────────────────────────────────────────────────────────────
# 1. Base system dependencies
# ─────────────────────────────────────────────────────────────────
log "Checking base dependencies (curl, git, build-essential, pkg-config)..."
if command_exists apt-get; then
    if ! dpkg -s build-essential pkg-config curl git >/dev/null 2>&1; then
        if [[ ! -t 0 ]] || ! sudo -n true 2>/dev/null; then
            warn "System packages missing but no interactive sudo. Install manually:"
            warn "  sudo apt-get update && sudo apt-get install -y build-essential pkg-config curl git"
        else
            log "Installing system packages via apt (may prompt for sudo)..."
            sudo apt-get update
            sudo apt-get install -y build-essential pkg-config curl git
        fi
    else
        info "System packages already present"
    fi
elif command_exists dnf; then
    if ! rpm -q gcc gcc-c++ pkgconfig curl git >/dev/null 2>&1; then
        log "Installing system packages via dnf (may prompt for sudo)..."
        sudo dnf install -y gcc gcc-c++ pkgconfig curl git
    else
        info "System packages already present"
    fi
elif command_exists pacman; then
    if ! pacman -Q base-devel pkgconf curl git >/dev/null 2>&1; then
        log "Installing system packages via pacman (may prompt for sudo)..."
        sudo pacman -S --needed base-devel pkgconf curl git
    else
        info "System packages already present"
    fi
else
    warn "No supported package manager found; assuming deps are installed"
fi

# ─────────────────────────────────────────────────────────────────
# 2. Rust toolchain
# ─────────────────────────────────────────────────────────────────
log "Checking Rust toolchain..."
if ! command_exists cargo; then
    log "Rust not found. Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
fi

RUST_TOOLCHAIN="stable"
if [[ -f "$ROOT_DIR/rust-toolchain.toml" ]]; then
    RUST_TOOLCHAIN=$(grep '^channel' "$ROOT_DIR/rust-toolchain.toml" | awk -F'"' '{print $2}')
fi

log "Installing Rust toolchain: $RUST_TOOLCHAIN with clippy, rustfmt, rust-analyzer..."
rustup toolchain install "$RUST_TOOLCHAIN" --component clippy --component rustfmt --component rust-analyzer 2>/dev/null || \
    rustup component add --toolchain "$RUST_TOOLCHAIN" clippy rustfmt rust-analyzer
rustup default "$RUST_TOOLCHAIN"

# Optional coverage tooling (skipped with --minimal)
if [[ "$MINIMAL" == false ]]; then
    log "Checking cargo-llvm-cov..."
    if ! command_exists cargo-llvm-cov; then
        log "Installing cargo-llvm-cov via rustup-bundled or taiki-e installer..."
        cargo install cargo-llvm-cov || true
    fi
fi

# ─────────────────────────────────────────────────────────────────
# 3. Rust workspace build and test
# ─────────────────────────────────────────────────────────────────
log "Building Rust workspace..."
cargo build --workspace
cargo test --workspace --quiet

log "Running Rust lint checks..."
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings

# ─────────────────────────────────────────────────────────────────
# 4. Python environment via uv
# ─────────────────────────────────────────────────────────────────
log "Checking uv..."
if ! command_exists uv; then
    log "uv not found. Installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    # shellcheck source=/dev/null
    source "$HOME/.local/bin/env" 2>/dev/null || true
fi

log "Creating/syncing Python virtualenv with uv..."
uv sync

if [[ -d "$VENV_DIR" ]]; then
    log "Virtualenv ready at $VENV_DIR"
else
    warn "Expected virtualenv at $VENV_DIR not found"
fi

log "Verifying Python dependencies..."
uv run python - <<'PY'
import numpy
import pandas
import matplotlib
import ppigrf
print(f"numpy={numpy.__version__} pandas={pandas.__version__} "
      f"matplotlib={matplotlib.__version__} ppigrf=OK")
PY

# ─────────────────────────────────────────────────────────────────
# 5. Fetch data assets
# ─────────────────────────────────────────────────────────────────
if [[ "$SKIP_DATA" == false ]]; then
    log "Fetching reference data..."
    ./scripts/fetch_data.sh
else
    info "Skipping data fetch"
fi

# ─────────────────────────────────────────────────────────────────
# 6. Summary
# ─────────────────────────────────────────────────────────────────
echo ""
echo "========================================="
log "Apogee setup complete!"
echo "========================================="
echo ""

if [[ "$SOURCED" == true && -d "$VENV_DIR" ]]; then
    log "Activating virtualenv in current shell..."
    # shellcheck source=/dev/null
    source "$VENV_DIR/bin/activate"
    log "Virtualenv active. Run 'deactivate' to exit."
else
    echo "Activate the virtualenv with:"
    echo "  source $VENV_DIR/bin/activate"
    echo "  # or use 'uv run' for one-off commands:"
    echo "  uv run python scripts/..."
fi

echo ""
echo "What you can do now:"
echo "  cargo build --workspace       # Build all Rust crates"
echo "  cargo test --workspace        # Run Rust tests"
echo "  cargo run -p apogee-server    # Run the headless server"
echo "  python scripts/...           # Run Python helper scripts (after activating venv)"
echo ""

# When sourced, do not exit so the activation survives in the caller's shell.