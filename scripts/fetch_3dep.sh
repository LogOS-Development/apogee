#!/bin/bash
# Fetch USGS 3DEP elevation tiles for a bounding box via The National Map API.
#
# Downloads 1/3 arc-second (~10m) GeoTIFF tiles and concatenates metadata.
# Tiles are stored in the specified output directory.
#
# Usage:
#   ./scripts/fetch_3dep.sh [output_dir] [bbox]
#
#   output_dir  — target directory (default: data/terrain/3dep)
#   bbox        — "min_lon,min_lat,max_lon,max_lat" (default: Boulder County CO)
#
# Examples:
#   ./scripts/fetch_3dep.sh                          # Boulder County, default
#   ./scripts/fetch_3dep.sh data/terrain "[-105.3,40.0,-105.2,40.1]"
#
# The TNM API returns JSON with download URLs. We extract the TIFF URLs
# and download each tile.
set -euo pipefail

OUTPUT_DIR="${1:-data/terrain/3dep}"
BBOX="${2:--105.3,40.0,-105.2,40.1}"

# Remove brackets if present
BBOX="${BBOX//[/}"
BBOX="${BBOX//]/}"

API_BASE="https://tnmaccess.nationalmap.gov/api/v1/products"
DATASETS="Digital Elevation Model (DEM) 1/3 arc-second"
PAGE_SIZE="1000"

mkdir -p "${OUTPUT_DIR}"

echo "Fetching 3DEP tiles for bbox [${BBOX}]..."

# Query TNM API for available tiles in the bbox
QUERY_URL="${API_BASE}?datasets=${DATASETS// /%20}&bbox=${BBOX}&max=${PAGE_SIZE}&format=json"

RESPONSE_FILE=$(mktemp)
trap 'rm -f "${RESPONSE_FILE}"' EXIT

curl -sS "${QUERY_URL}" -o "${RESPONSE_FILE}"

if [ ! -s "${RESPONSE_FILE}" ]; then
    echo "ERROR: empty response from TNM API"
    exit 1
fi

# Extract download URLs for GeoTIFF tiles using Python
TILE_COUNT=$(python3 -c "
import json, sys

with open('${RESPONSE_FILE}') as f:
    data = json.load(f)

items = data.get('items', [])
if not items:
    print('ERROR: no tiles found for this bbox', file=sys.stderr)
    sys.exit(1)

urls = []
for item in items:
    for dl in item.get('downloadUrls', []):
        if dl.get('url', '').endswith('.tif') or dl.get('url', '').endswith('.tiff'):
            urls.append(dl['url'])

if not urls:
    print('ERROR: no GeoTIFF download URLs found', file=sys.stderr)
    sys.exit(1)

print(len(urls))
for u in urls:
    print(u)
" 2>&1)

# First line is count, rest are URLs
URLS=$(echo "${TILE_COUNT}" | tail -n +2)
COUNT=$(echo "${TILE_COUNT}" | head -1)

echo "Found ${COUNT} tiles. Downloading to ${OUTPUT_DIR}/..."

DOWNLOADED=0
SKIPPED=0
for url in ${URLS}; do
    FILENAME=$(basename "${url}")
    TARGET="${OUTPUT_DIR}/${FILENAME}"

    if [ -f "${TARGET}" ]; then
        echo "  SKIP (exists): ${FILENAME}"
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    echo "  Downloading: ${FILENAME}"
    curl -sS -o "${TARGET}" "${url}" || {
        echo "  FAILED: ${FILENAME}"
        rm -f "${TARGET}"
    }

    DOWNLOADED=$((DOWNLOADED + 1))
done

echo "Done. ${DOWNLOADED} downloaded, ${SKIPPED} skipped (already present)."
echo "Tiles in ${OUTPUT_DIR}/"
ls -lh "${OUTPUT_DIR}/" | head -20