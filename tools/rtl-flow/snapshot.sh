#!/usr/bin/env bash
set -euo pipefail
main() {
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
output=$(realpath -m "${1:?Usage: snapshot.sh NEW_OUTPUT_DIRECTORY}")
pdma_new_run "$output"
cd "$root"
GIT_MASTER=1 bash "$root/tools/rtl-flow/veryl.sh" --version > "$output/compiler.txt"
(cd "$root/rtl" && GIT_MASTER=1 bash "$root/tools/rtl-flow/veryl.sh" metadata --format json --format-version 2) \
    > "$output/metadata-v2.json"
design_inputs() {
    # Only compiler inputs: README edits do not invalidate a hardware snapshot.
    # Recompute the inventory too, so adding/removing a source during the build
    # cannot evade the final check of the previously listed files.
    python3 "$root/tools/rtl-flow/ip/bundle.py" inputs "$output/metadata-v2.json"
    find tools/rtl-flow/patches -type f -name '*.patch' -print0 | sort -z | xargs -0 sha256sum
    sha256sum tools/rtl-flow/toolchain.env tools/rtl-flow/veryl.sh \
        tools/rtl-flow/snapshot.sh tools/rtl-flow/ip/bundle.py
}
design_inputs > "$output/design.sha256"
python3 "$root/tools/rtl-flow/ip/bundle.py" bundle "$output/metadata-v2.json" "$output/resources"
mv "$output/resources/ip" "$output/resources/ip-manifest.tcl" "$output/resources/ip-manifest.json" "$output/"
rmdir "$output/resources"
cd "$root/rtl"
bash "$root/tools/rtl-flow/veryl.sh" build > "$output/build.log" 2>&1
bash "$root/tools/rtl-flow/veryl.sh" --version > "$output/compiler.txt"
index=0
while IFS= read -r source; do
    [[ $source == */target/tests/* ]] && continue
    [[ -f $source ]] || { printf 'Missing emitted source: %s\n' "$source" >&2; exit 1; }
    file=$(printf '%03d_%s' "$index" "$(basename "$source")")
    cp "$source" "$output/$file"
    printf '%s\n' "$output/$file" >> "$output/sources.f"
    ((index+=1))
done < pdma_rtl.f
((index>0))
cd "$root"
design_inputs > "$output/design-after.sha256"
cmp "$output/design.sha256" "$output/design-after.sha256"
cd "$output"
find . -type f \( -name '*.sv' -o -path './ip/*' -o -name 'ip-manifest.*' \) -print0 \
    | sort -z | xargs -0 sha256sum > snapshot.sha256
sha256sum sources.f compiler.txt design.sha256 metadata-v2.json >> snapshot.sha256
printf 'PDMA_SNAPSHOT_COMPLETE\n'
}
{ main "$@"; exit $?; }
