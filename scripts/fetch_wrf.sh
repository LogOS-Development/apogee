#!/bin/bash
# Fetch WRF v4.8.0 source for the wrf FFI feature.
# Downloads only the physics schemes needed, not the full repo.
#
# Usage: ./scripts/fetch_wrf.sh
set -euo pipefail

WRF_VERSION="v4.8.0"
WRF_DIR="$(dirname "$0")/../crates/apogee-core/external/wrf/vendor"
PHYS_DIR="${WRF_DIR}/phys"

if [ -d "${PHYS_DIR}" ] && [ -f "${PHYS_DIR}/module_mp_kessler.F" ]; then
    echo "WRF source already present at ${WRF_DIR}"
    exit 0
fi

echo "Fetching WRF ${WRF_VERSION} source..."
mkdir -p "${WRF_DIR}"

# Shallow clone the release tag, then remove .git to save space.
git clone --depth 1 --branch "${WRF_VERSION}" \
    https://github.com/wrf-model/WRF.git "${WRF_DIR}" 2>&1 | tail -3

# Prune unnecessary directories to reduce footprint.
rm -rf "${WRF_DIR}/.git" \
       "${WRF_DIR}/chem" \
       "${WRF_DIR}/hydro" \
       "${WRF_DIR}/var" \
       "${WRF_DIR}/wrftladj" \
       "${WRF_DIR}/test" \
       "${WRF_DIR}/tools" \
       "${WRF_DIR}/doc" \
       "${WRF_DIR}/run"

echo "WRF source fetched to ${WRF_DIR}"
du -sh "${WRF_DIR}"