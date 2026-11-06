#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$here/toolchain.env"
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
    printf 'This qualified tool package requires Linux x86_64.\n' >&2
    exit 1
fi
cache="$here/.run/tools"
archive="$cache/veryl-$VERYL_COMMIT.tar.gz"
installed="$cache/veryl-$VERYL_COMMIT-rust$VERYL_RUST_VERSION-$VERYL_BUILD_PROFILE-p${VERYL_PATCH_SHA256:0:12}"
patch_manifest() {
    for patch_name in "${VERYL_PATCH_FILES[@]}"; do
        (cd "$here/patches" && sha256sum "$patch_name")
    done
}
digest=$(patch_manifest | sha256sum)
[[ ${digest%% *} == "$VERYL_PATCH_SHA256" ]] || { printf 'Veryl patch manifest checksum mismatch\n' >&2; exit 1; }
mkdir -p "$cache"
exec 9>"$cache/install.lock"
flock 9
if [[ ! -d "$installed" ]]; then
    rust_version=$(rustup run "$VERYL_RUST_VERSION" rustc --version)
    if [[ $rust_version != "rustc $VERYL_RUST_VERSION "* ]]; then
        printf 'Building Veryl requires Rust %s; found %s\n' "$VERYL_RUST_VERSION" "$rust_version" >&2
        exit 1
    fi
    if [[ ! -f "$archive" ]]; then
        curl -fL --retry 3 --retry-all-errors --connect-timeout 10 --max-time 300 \
            -o "$archive.part" \
            "https://codeload.github.com/veryl-lang/veryl/tar.gz/$VERYL_COMMIT"
        digest=$(sha256sum "$archive.part")
        [[ ${digest%% *} == "$VERYL_SOURCE_SHA256" ]] || exit 1
        mv "$archive.part" "$archive"
    fi
    digest=$(sha256sum "$archive")
    if [[ ${digest%% *} != "$VERYL_SOURCE_SHA256" ]]; then
        printf 'Veryl source checksum mismatch: %s\n' "$archive" >&2
        exit 1
    fi
    staging=$(mktemp -d "$cache/veryl-build.XXXXXXXX")
    trap 'rm -rf -- "$staging"' EXIT
    mkdir "$staging/source" "$staging/install" "$staging/install/bin"
    tar -xzf "$archive" --strip-components=1 -C "$staging/source"
    for patch_name in "${VERYL_PATCH_FILES[@]}"; do
        patch --batch --fuzz=0 -d "$staging/source" -p1 -i "$here/patches/$patch_name" >&2
    done
    target=$(realpath -m "${CARGO_TARGET_DIR:-$cache/veryl-target}")
    (
        cd "$staging/source"
        VERSION="$VERYL_VERSION" CARGO_TARGET_DIR="$target" \
            rustup run "$VERYL_RUST_VERSION" cargo build --locked -p veryl \
            --profile "$VERYL_BUILD_PROFILE" -j "${VERYL_BUILD_JOBS:-4}" >&2
    )
    install -m 755 "$target/$VERYL_BUILD_PROFILE/veryl" "$staging/install/bin/veryl"
    printf 'commit=%s\nsource_sha256=%s\nrust=%s\nprofile=%s\npatch_sha256=%s\n' \
        "$VERYL_COMMIT" "$VERYL_SOURCE_SHA256" "$rust_version" "$VERYL_BUILD_PROFILE" "$VERYL_PATCH_SHA256" \
        > "$staging/install/provenance.txt"
    (cd "$staging/install" && sha256sum bin/veryl > SHA256SUMS)
    patch_manifest > "$staging/install/PATCHES.sha256"
    mv "$staging/install" "$installed"
    rm -rf -- "$staging"
    trap - EXIT
fi
(cd "$installed" && sha256sum --check --status SHA256SUMS)
flock -u 9
exec 9>&-
if [[ ${1:-} == --print-bin-dir ]]; then
    printf '%s/bin\n' "$installed"
else
    exec "$installed/bin/veryl" "$@"
fi
