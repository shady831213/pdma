#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source_dir=$(realpath "${1:?Usage: prepare-jarvisuk.sh SOURCE NEW_DEST}")
dest=$(realpath -m "${2:?Usage: prepare-jarvisuk.sh SOURCE NEW_DEST}")
pin="$root/verification/dma/dependencies/jarvisuk.lock.json"
allocator=src/memory/jvs_memory_allocator.sv
read -r revision expected < <(python3 - "$pin" <<'PY'
import json, sys
with open(sys.argv[1]) as file:
    pin = json.load(file)
print(pin["revision"], pin["allocator_sha256"])
PY
)
[[ $revision =~ ^[0-9a-f]{40}$ && $expected =~ ^[0-9a-f]{64}$ ]]
if [[ -e "$dest" || -L "$dest" || "$dest/" == "$source_dir/"* ]]; then
    printf 'Destination must be new and outside the source directory\n' >&2
    exit 1
fi
if ! GIT_MASTER=1 git -C "$source_dir" cat-file -e "$revision^{commit}" 2>/dev/null; then
    printf 'Pinned jarvisuk revision is unavailable; fetch the dependency checkout first\n' >&2
    exit 1
fi
actual=$(GIT_MASTER=1 git -C "$source_dir" show "$revision:$allocator" | sha256sum)
if [[ "${actual%% *}" != "$expected" ]]; then
    printf 'Pinned jarvisuk allocator checksum mismatch\n' >&2
    exit 1
fi
mkdir -p "$(dirname "$dest")"
mkdir "$dest"
GIT_MASTER=1 git -C "$source_dir" archive --format=tar "$revision" | tar -C "$dest" -xf -
(
    cd "$dest"
    [[ -f LICENSE && -f etc/jvs.f ]]
    printf '%s  %s\n' "$expected" "$allocator" | sha256sum --check --status
    printf '%s\n' "$revision" > pdma-jarvisuk-revision.txt
    sha256sum "$pin" > pdma-jarvisuk-pin.sha256
)
printf 'PDMA_JARVISUK_PREPARED revision=%s\n' "$revision"
