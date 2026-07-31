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

echo "Apogee data acquisition script"
echo "Data directory: $DATA_DIR"
echo ""

# TODO: implement download + validation
# Each asset should:
#   1. Download to the correct subdirectory
#   2. Verify file hash or format
#   3. Print status

echo "WARNING: fetch_data.sh is a stub — Phase 0.2 not yet implemented"
exit 1