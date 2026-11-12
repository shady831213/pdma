#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
export PDMA_DESIGN_PROFILE=${PDMA_DESIGN_PROFILE:-$root/impl/dma/profiles/copy.tcl}
exec bash "$root/tools/rtl-flow/implementation/genus.sh" "$@"
