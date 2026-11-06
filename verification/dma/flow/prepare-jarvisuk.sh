#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source_dir=$(realpath "${1:?Usage: prepare-jarvisuk.sh SOURCE NEW_DEST}")
dest=$(realpath -m "${2:?Usage: prepare-jarvisuk.sh SOURCE NEW_DEST}")
patch_file="$root/verification/dma/dependencies/patches/jarvisuk-allocator-free.patch"
allocator=src/memory/jvs_memory_allocator.sv
expected=8e893c07f27538a586687b00692db982004ecfa775280b47d73b57d936d0f9ee
[[ -f "$source_dir/LICENSE" && -f "$source_dir/etc/jvs.f" ]]
if [[ -e "$dest" || -L "$dest" || "$dest/" == "$source_dir/"* ]]; then
    printf 'Destination must be new and outside the source directory\n' >&2
    exit 1
fi
actual=$(sha256sum "$source_dir/$allocator")
if [[ "${actual%% *}" != "$expected" ]]; then
    printf 'Unsupported jarvisuk allocator source checksum\n' >&2
    exit 1
fi
mkdir -p "$(dirname "$dest")"
mkdir "$dest"
tar -C "$source_dir" --exclude=.git -cf - . | tar -C "$dest" -xf -
(
    cd "$dest"
    printf '%s  %s\n' "$expected" "$allocator" | sha256sum --check --status
    sha256sum "$allocator" > pdma-allocator-before.sha256
    sha256sum "$patch_file" > pdma-allocator-patch.sha256
    patch --batch --forward --fuzz=0 -p1 < "$patch_file" > pdma-allocator-patch.log
    printf '%s  %s\n' e85bb69f54ca1f756996aee29d530a6b82bad53d4cdefcd91e05f0117372668b "$allocator" | sha256sum --check --status
    sha256sum "$allocator" > pdma-allocator-after.sha256
)
printf 'PDMA_JARVISUK_PREPARED\n'
