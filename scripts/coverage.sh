#!/usr/bin/env bash
# Run code coverage locally and print a line-coverage summary.
#
# This script detects a system LLVM installation when rustup's
# llvm-tools-preview component is not available (e.g. distro-packaged Rust).

set -euo pipefail

cd "$(dirname "$0")/.."

OUTPUT_PATH="${1:-lcov.info}"

# Find a system llvm-cov / llvm-profdata if rustup's preview tools are absent.
if command -v llvm-cov >/dev/null 2>&1; then
    export LLVM_COV="$(command -v llvm-cov)"
    export LLVM_PROFDATA="$(command -v llvm-profdata || true)"
else
    for v in 21 20 19 18 17 16 15 14; do
        if [ -x "/usr/lib/llvm-${v}/bin/llvm-cov" ]; then
            export LLVM_COV="/usr/lib/llvm-${v}/bin/llvm-cov"
            export LLVM_PROFDATA="/usr/lib/llvm-${v}/bin/llvm-profdata"
            break
        fi
    done
fi

if [ -z "${LLVM_COV:-}" ]; then
    echo "error: could not find llvm-cov; install llvm-tools-preview or a system LLVM" >&2
    exit 1
fi

echo "Using LLVM_COV=$LLVM_COV"

cargo llvm-cov --workspace --lcov --output-path "$OUTPUT_PATH"

STATS=$(grep "^DA:" "$OUTPUT_PATH" | awk -F, '{total++; if ($2>0) hit++} END {printf "%d %d", total, hit}')
TOTAL_FOUND=$(echo "$STATS" | awk '{print $1}')
TOTAL_HIT=$(echo "$STATS" | awk '{print $2}')
PCT=$(echo "scale=1; $TOTAL_HIT * 100 / $TOTAL_FOUND" | bc)

# Color: red <60, orange <80, yellow <90, green >=90
if (( $(echo "$PCT >= 90" | bc -l) )); then
    COLOR="4c1"
elif (( $(echo "$PCT >= 80" | bc -l) )); then
    COLOR="dfb317"
elif (( $(echo "$PCT >= 60" | bc -l) )); then
    COLOR="fe7d37"
else
    COLOR="e05d44"
fi

cat > .github/coverage.json <<EOF
{
  "schemaVersion": 1,
  "label": "coverage",
  "message": "${PCT}%",
  "color": "$COLOR"
}
EOF

echo "Coverage: ${PCT}% (${TOTAL_HIT}/${TOTAL_FOUND} lines)"
echo "LCOV report: ${OUTPUT_PATH}"
echo "Shield JSON: .github/coverage.json"
