#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
source "$here/toolchain.env"
launcher="$root/external/my-veryl/veryl.sh"
if [[ ! -f "$launcher" ]]; then
    printf 'Initialize the pinned compiler helper: GIT_MASTER=1 git submodule update --init external/my-veryl\n' >&2
    exit 2
fi
export MY_VERYL_CACHE_DIR=${MY_VERYL_CACHE_DIR:-$here/.run/tools}
exec bash "$launcher" --toolchain "$MY_VERYL_TOOLCHAIN" "$@"
