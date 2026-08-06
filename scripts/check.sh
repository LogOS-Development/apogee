#!/usr/bin/env bash
# Run the full local quality gate: format check, clippy, tests, and coverage.
#
# Usage:
#   scripts/check.sh                  # default features, quiet output
#   scripts/check.sh --all-features    # include hwm14 FFI (needs gfortran)
#   scripts/check.sh -v                # verbose (show full cargo output)
#   scripts/check.sh -v --all-features
#
# Quiet mode prints only warnings/errors and the cumulative summary.
# Exits non-zero if any stage fails.

set -euo pipefail

cd "$(dirname "$0")/.."

ALL_FEATURES=0
VERBOSE=0
for arg in "$@"; do
    case "$arg" in
        --all-features) ALL_FEATURES=1 ;;
        -v|--verbose) VERBOSE=1 ;;
    esac
done

if [ "$ALL_FEATURES" -eq 1 ]; then
    FEATURE_FLAG="--all-features"
    FEATURE_LABEL="all features"
else
    FEATURE_FLAG=""
    FEATURE_LABEL="default features"
fi

PASS=0
FAIL=0
RESULTS=()

record() {
    local name="$1" status="$2" detail="${3:-}"
    if [ "$status" = "PASS" ]; then
        PASS=$((PASS + 1))
        RESULTS+=("  PASS  $name  $detail")
    else
        FAIL=$((FAIL + 1))
        RESULTS+=("  FAIL  $name  $detail")
    fi
}

separator() {
    printf '=%.0s' {1..60}
    echo
}

run_cmd() {
    if [ "$VERBOSE" -eq 1 ]; then
        "$@"
    else
        "$@" 2>&1 | grep -vE '^(     Running |   Compiling |    Checking |    Finished |warning:|note:|help:|  -->|   \| |^\s*$)' || true
    fi
}

# ─────────────────────────────────────────────────────────────────
# 1. Format check
# ─────────────────────────────────────────────────────────────────
echo ""
separator
echo "  1/4  Format check (cargo fmt --check)"
separator
FMT_OUTPUT=$(cargo fmt --all -- --check 2>&1) || true
if [ -z "$FMT_OUTPUT" ]; then
    record "Format" "PASS"
else
    record "Format" "FAIL" "(run: cargo fmt --all)"
    if [ "$VERBOSE" -eq 0 ]; then
        # Show just which files need formatting
        echo "$FMT_OUTPUT" | grep '^Diff in' | head -20
    else
        echo "$FMT_OUTPUT"
    fi
fi

# ─────────────────────────────────────────────────────────────────
# 2. Clippy
# ─────────────────────────────────────────────────────────────────
echo ""
separator
echo "  2/4  Clippy ($FEATURE_LABEL)"
separator
CLIPPY_OUTPUT=$(cargo clippy --workspace $FEATURE_FLAG -- -D warnings 2>&1) || CLIPPY_FAIL=1
if [ "${CLIPPY_FAIL:-0}" -eq 0 ]; then
    record "Clippy" "PASS" "$FEATURE_LABEL"
else
    record "Clippy" "FAIL" "$FEATURE_LABEL"
    if [ "$VERBOSE" -eq 0 ]; then
        echo "$CLIPPY_OUTPUT" | grep -E '^(error|warning)' | head -20
    else
        echo "$CLIPPY_OUTPUT"
    fi
fi

# ─────────────────────────────────────────────────────────────────
# 3. Tests
# ─────────────────────────────────────────────────────────────────
echo ""
separator
echo "  3/4  Tests ($FEATURE_LABEL)"
separator
TEST_OUTPUT=$(cargo test --workspace $FEATURE_FLAG 2>&1)

if [ "$VERBOSE" -eq 1 ]; then
    echo "$TEST_OUTPUT"
else
    # Show only failures and the aggregate per-crate result lines with actual tests
    echo "$TEST_OUTPUT" | grep -E '^(test result:.*[1-9]+ (passed|failed)|FAILED|failures:)' | head -20
    echo "$TEST_OUTPUT" | grep -E '\bFAIL\b' | head -20 || true
fi

# Sum all test result lines across all crates
TOTAL_PASSED=0
TOTAL_FAILED=0
TOTAL_IGNORED=0
while IFS= read -r line; do
    p=$(echo "$line" | grep -oP '\d+(?= passed)' || echo "0")
    f=$(echo "$line" | grep -oP '\d+(?= failed)' || echo "0")
    i=$(echo "$line" | grep -oP '\d+(?= ignored)' || echo "0")
    TOTAL_PASSED=$((TOTAL_PASSED + p))
    TOTAL_FAILED=$((TOTAL_FAILED + f))
    TOTAL_IGNORED=$((TOTAL_IGNORED + i))
done <<< "$(echo "$TEST_OUTPUT" | grep '^test result:')"

if [ "$TOTAL_FAILED" -eq 0 ]; then
    record "Tests" "PASS" "$TOTAL_PASSED passed, $TOTAL_FAILED failed, $TOTAL_IGNORED ignored"
else
    record "Tests" "FAIL" "$TOTAL_PASSED passed, $TOTAL_FAILED failed, $TOTAL_IGNORED ignored"
fi

# ─────────────────────────────────────────────────────────────────
# 4. Coverage
# ─────────────────────────────────────────────────────────────────
echo ""
separator
echo "  4/4  Coverage ($FEATURE_LABEL)"
separator

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

LCOV_PATH="/tmp/apogee-lcov.info"
COV_OUTPUT=$(cargo llvm-cov --workspace $FEATURE_FLAG --lcov --output-path "$LCOV_PATH" 2>&1) || COV_FAIL=1
if [ "${COV_FAIL:-0}" -eq 0 ]; then
    STATS=$(grep "^DA:" "$LCOV_PATH" | awk -F, '{total++; if ($2>0) hit++} END {printf "%d %d", total, hit}')
    TOTAL_FOUND=$(echo "$STATS" | awk '{print $1}')
    TOTAL_HIT=$(echo "$STATS" | awk '{print $2}')
    if [ "$TOTAL_FOUND" -gt 0 ]; then
        PCT=$(echo "scale=1; $TOTAL_HIT * 100 / $TOTAL_FOUND" | bc)
    else
        PCT="0.0"
    fi
    record "Coverage" "PASS" "${PCT}% (${TOTAL_HIT}/${TOTAL_FOUND} lines)"
else
    record "Coverage" "FAIL" "llvm-cov error"
    if [ "$VERBOSE" -eq 0 ]; then
        echo "$COV_OUTPUT" | grep -E '^(error|warning)' | head -10
    else
        echo "$COV_OUTPUT"
    fi
    PCT="N/A"
fi

# ─────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────
echo ""
separator
echo "  SUMMARY"
separator
printf '%s\n' "${RESULTS[@]}"
echo ""
if [ "$FAIL" -eq 0 ]; then
    echo "  All checks passed ($PASS/$PASS)  —  $TOTAL_PASSED tests, ${PCT}% coverage"
else
    echo "  $FAIL check(s) failed, $PASS passed  —  $TOTAL_PASSED tests, ${PCT}% coverage"
fi
echo ""

exit "$FAIL"