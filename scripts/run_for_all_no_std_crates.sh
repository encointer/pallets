#!/usr/bin/env bash
set -euo pipefail

# Colors for CI logs
RED='\033[1;31m'
GREEN='\033[1;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # reset

export RUSTFLAGS="${RUSTFLAGS:-} --cfg substrate_runtime"

# First arg is the cargo command (e.g., check, build)
COMMAND="$1"
shift || true

status=0
FAILED_CRATES=()
PASSED_CRATES=()

while IFS= read -r CARGO_TOML; do
    DIR=$(dirname "$CARGO_TOML")
    CRATE_NAME=$(awk '
      /^\[package\]/{flag=1; next}
      /^\[/{flag=0}
      flag && /^name =/ {
        gsub(/"/,"",$3);
        print $3;
        exit
      }
    ' "$CARGO_TOML")

    if [ -z "$CRATE_NAME" ]; then
      CRATE_NAME=$(basename "$DIR")
    fi

    echo "::group::[crate:$CRATE_NAME] Building $CRATE_NAME"
    echo -e "${YELLOW}==> Checking in directory:${NC} $DIR"

    # Skip if no `std` feature
    if ! grep -q "\[features\]" "$CARGO_TOML" || ! grep -q "std = \[" "$CARGO_TOML"; then
        echo -e "${YELLOW}    Skipping:${NC} no 'std' feature found."
        echo "::endgroup::"
        continue
    fi

    # Determine if runtime-benchmarks feature should be added
    if grep -q "\[features\]" "$CARGO_TOML" && grep -q "runtime-benchmarks = \[" "$CARGO_TOML"; then
        echo -e "${GREEN}    Found:${NC} runtime-benchmarks feature. Running with it..."
        if ! cargo "$COMMAND" "$@" \
            --features runtime-benchmarks \
            --manifest-path "$CARGO_TOML"; then
            >&2 echo -e "${RED}    FAILED:${NC} $DIR"
            FAILED_CRATES+=("$CRATE_NAME")
            status=1
        else
            echo -e "${GREEN}    OK:${NC} $DIR"
            PASSED_CRATES+=("$CRATE_NAME")
        fi
    else
        echo -e "${YELLOW}    No runtime-benchmarks feature. Running without it...${NC}"
        if ! cargo "$COMMAND" "$@" \
            --manifest-path "$CARGO_TOML"; then
            >&2 echo -e "${RED}    FAILED:${NC} $DIR"
            FAILED_CRATES+=("$CRATE_NAME")
            status=1
        else
            echo -e "${GREEN}    OK:${NC} $DIR"
            PASSED_CRATES+=("$CRATE_NAME")
        fi
    fi
    echo "::endgroup::"
done < <(find . -name "Cargo.toml")

# Summary table
echo ""
echo "====================== Summary ======================"
if [ "${#PASSED_CRATES[@]}" -gt 0 ]; then
    sorted_passed=($(printf "%s\n" "${PASSED_CRATES[@]}" | sort))
    echo -e "${GREEN}PASSED:${NC} ${sorted_passed[*]}"
fi

if [ "${#FAILED_CRATES[@]}" -gt 0 ]; then
    sorted_failed=($(printf "%s\n" "${FAILED_CRATES[@]}" | sort))
    echo -e "${RED}FAILED:${NC} ${sorted_failed[*]}"
    echo ""
    echo "Failed crate logs are grouped in the Actions log with headers like:"
    for crate in "${sorted_failed[@]}"; do
        echo "  [crate:${crate}]"
    done
    echo ""
    echo "Search the log output for these to jump directly to them."
fi
echo "======================================================"
if [ "$status" -ne 0 ]; then
    echo -e "${RED}One or more crates failed.${NC}"
    exit 1
else
    echo -e "${GREEN}All crates passed.${NC}"
fi
