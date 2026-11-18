#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
export PDMA_DESIGN_PROFILE=${PDMA_DESIGN_PROFILE:-$root/impl/dma/profiles/copy.tcl}
snapshot=$(realpath "${1:?Supply a configured RTL snapshot}")
export PDMA_RTL_CONFIGURATION="$snapshot/configuration.tcl"
[[ -f $PDMA_RTL_CONFIGURATION ]] || { printf 'Use rtl/flow/snapshot.sh for the DMA profile\n' >&2; exit 2; }
exec bash "$root/tools/rtl-flow/implementation/genus.sh" "$@"
