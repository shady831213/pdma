#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
if [[ $# != 1 || ($1 != --write && $1 != --check) ]]; then
    printf 'Usage: bash tools/rtl-flow/sv-layout.sh --write|--check\n' >&2
    exit 2
fi
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" cargo run --locked --quiet \
    --manifest-path "$root/model/modeling/Cargo.toml" \
    -p pdma-model-contract --features export-sv --bin pdma-sv-layout -- \
    "$1" "$root/verification/dma/tb/register_layout.sv"
