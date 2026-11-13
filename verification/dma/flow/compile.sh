#!/usr/bin/env bash
set -euo pipefail
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/process.sh"
    pdma_process_pid=
    trap pdma_stop_process EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    snapshot=$(realpath "${1:?Usage: compile.sh SNAPSHOT NEW_BUILD}")
    build=$(realpath -m "${2:?Usage: compile.sh SNAPSHOT NEW_BUILD}")
    channels=${PDMA_CHANNEL_COUNT:-2}
    [[ $channels =~ ^[1-8]$ ]] || { printf 'PDMA_CHANNEL_COUNT must be in 1..8\n' >&2; exit 2; }
    pdma_environment verification
    : "${SVT_COMMON_HOME:?Configure the VIP common installation}"
    : "${SVT_AMBA_HOME:?Configure the AXI VIP installation}"
    : "${JVSUK_HOME:?Configure the jarvisuk checkout}"
    : "${DESIGNWARE_HOME:?Configure the VIP installation root}"
    export JVSUK_HOME SVT_COMMON_HOME SVT_AMBA_HOME
    pdma_new_run "$build"
    printf 'channel_count=%s\n' "$channels" > "$build/configuration.txt"
    bash "$root/verification/dma/flow/prepare-jarvisuk.sh" "$JVSUK_HOME" "$build/jarvisuk"
    export JVSUK_HOME="$build/jarvisuk"
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    custody=$(python3 "$root/tools/rtl-flow/ip/bundle.py" locate "$snapshot" common_cell verification/cdc_custody.sv)
    cd "$root/verification/dma"
    find tb env tests checkers -type f -print0 | sort -z | xargs -0 sha256sum > "$build/testbench.sha256"
    find "$JVSUK_HOME/src" "$JVSUK_HOME/include" "$JVSUK_HOME/interfaces" "$JVSUK_HOME/pkg" "$JVSUK_HOME/etc" \
        -type f -print0 | sort -z | xargs -0 sha256sum > "$build/jarvisuk.sha256"
    find "$SVT_COMMON_HOME/sverilog" "$SVT_AMBA_HOME/sverilog" \
        -type f -print0 | sort -z | xargs -0 sha256sum > "$build/vip.sha256"
    sha256sum "$root/tools/rtl-flow/environment.sh" "$root/tools/rtl-flow/process.sh" "$root/verification/dma/flow/compile.sh" \
        "$root/tools/rtl-flow/ip/bundle.py" \
        "$root/verification/dma/flow/prepare-jarvisuk.sh" \
        "$root/verification/dma/dependencies/jarvisuk.lock.json" \
        "$snapshot/snapshot.sha256" "$build/configuration.txt" > "$build/flow.sha256"
    pdma_execute "${PDMA_COMPILE_TIMEOUT:-1200}" "$build/console.log" "${PDMA_SIM_COMPILER:-vcs}" \
        -full64 -sverilog -ntb_opts uvm-1.2 -timescale=1ns/1ps -j4 \
        "+define+PDMA_CHANNEL_COUNT=$channels" \
        "+define+DESIGNWARE_INCDIR=$DESIGNWARE_HOME" \
        -cm line+cond+tgl+branch -cm_dir "$build/coverage.vdb" \
        -f "$snapshot/sources.f" "$custody" -f tb/vip.f -top pdma_vip_top \
        -Mdir="$build/csrc" -o "$build/simv" -l "$build/compile.log"
    sha256sum --check --status "$build/testbench.sha256"
    sha256sum --check --status "$build/jarvisuk.sha256"
    sha256sum --check --status "$build/vip.sha256"
    sha256sum --check --status "$build/flow.sha256"
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    (cd "$build" && find simv simv.daidir -type f -print0 | sort -z | xargs -0 sha256sum > executable.sha256)
    printf 'PDMA_VIP_COMPILE_COMPLETE\n'
}
{ main "$@"; exit $?; }
