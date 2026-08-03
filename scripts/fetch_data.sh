#!/usr/bin/env bash
# Apogee data acquisition script (Phase 0.2)
#
# Fetches and validates all required data files:
#   - JPL DE441 ephemeris (NAIF/SPICE binary kernel)
#   - EGM2008 gravity model (NGA spherical harmonic coefficients)
#   - F10.7 / geomagnetic data (NOAA SWPC / Celestrak)
#   - Leap second table (IERS)
#   - Earth orientation params (IERS EOP C04)
#   - Sample TLEs (Celestrak / Space-Track)
#
# Usage: ./scripts/fetch_data.sh
# Exit criterion: data/ populated with validated files.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$ROOT_DIR/data"
MANIFEST="$ROOT_DIR/data/manifest.json"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

log()   { echo -e "${GREEN}[FETCH]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
fail()  { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Verify a file exists and has minimum size
verify_file() {
    local path="$1"
    local min_size="${2:-100}"

    if [[ ! -f "$path" ]]; then
        fail "Missing: $path"
    fi

    local size
    size=$(stat -c%s "$path" 2>/dev/null || stat -f%z "$path" 2>/dev/null)
    if (( size < min_size )); then
        fail "File too small ($size bytes < $min_size): $path"
    fi

    log "OK: $(basename "$path") ($size bytes)"
}

# Download with curl, only if file doesn't exist. Refuses HTML error pages
# (e.g. 404 served as text/html) by checking the first bytes.
download() {
    local url="$1"
    local dest="$2"

    if [[ -f "$dest" ]]; then
        log "Exists, skipping: $(basename "$dest")"
        return 0
    fi

    log "Downloading: $(basename "$dest")"
    curl -L -f -s -o "$dest" "$url" || fail "Download failed: $url"

    # Reject HTML error pages that slipped through.
    if file "$dest" | grep -q 'HTML'; then
        rm -f "$dest"
        fail "Downloaded file is HTML, not data: $url"
    fi
}

echo "========================================="
echo "Apogee Data Acquisition Script"
echo "Data directory: $DATA_DIR"
echo "========================================="
echo ""

# Create directories
mkdir -p "$DATA_DIR"/{ephemeris,gravity,spaceweather,time,eop}
mkdir -p "$ROOT_DIR/tests/fixtures"

# --- 1. JPL DE441 Ephemeris ---
log "=== JPL DE441 Ephemeris ==="
download \
    "https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp/de441.bsp" \
    "$DATA_DIR/ephemeris/de441.bsp"
verify_file "$DATA_DIR/ephemeris/de441.bsp" 1000000

# --- 2. EGM2008 Gravity Model ---
log "=== EGM2008 Gravity Model ==="
# The original NGA .gz URL is permanently gone. Use the ICGEM .gfc mirror,
# which the spherical-harmonics loader now supports.
download \
    "https://icgem.gfz-potsdam.de/getmodel/gfc/c50128797a9cb62e936337c890e4425f03f0461d7329b09a8cc8561504465340/EGM2008.gfc" \
    "$DATA_DIR/gravity/EGM2008_2190_TideFree.gfc"
verify_file "$DATA_DIR/gravity/EGM2008_2190_TideFree.gfc" 100000000

# --- 3. Leap Second Table ---
log "=== Leap Second Table ==="
download \
    "https://hpiers.obspm.fr/iers/bul/bulc/Leap_Second.dat" \
    "$DATA_DIR/time/Leap_Second.dat"
verify_file "$DATA_DIR/time/Leap_Second.dat" 50

# --- 4. Earth Orientation Parameters (EOP C04) ---
log "=== EOP C04 ==="
download \
    "https://hpiers.obspm.fr/iers/eop/eopc04/eopc04.1962-now" \
    "$DATA_DIR/eop/eopc04.txt"
verify_file "$DATA_DIR/eop/eopc04.txt" 1000

# --- 5. Space Weather (F10.7, Ap/Kp) ---
log "=== Space Weather Data ==="
download \
    "https://celestrak.org/SpaceData/SW-All.txt" \
    "$DATA_DIR/spaceweather/SW-All.txt"
verify_file "$DATA_DIR/spaceweather/SW-All.txt" 1000

# --- 6. Sample TLEs ---
log "=== Sample TLEs ==="
download \
    "https://celestrak.org/NORAD/elements/gp.php?GROUP=stations&FORMAT=tle" \
    "$ROOT_DIR/tests/fixtures/iss_tle.txt"
verify_file "$ROOT_DIR/tests/fixtures/iss_tle.txt" 100

# --- Summary ---
echo ""
echo "========================================="
log "Data acquisition complete!"
echo "========================================="
echo ""
echo "Files in data/:"
find "$DATA_DIR" -type f -exec ls -lh {} \; 2>/dev/null || true
echo ""
echo "Test fixtures:"
find "$ROOT_DIR/tests/fixtures" -type f -exec ls -lh {} \; 2>/dev/null || true