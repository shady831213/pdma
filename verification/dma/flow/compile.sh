#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
snapshot=$(realpath "${1:?Usage: compile.sh SNAPSHOT NEW_BUILD}")
build=$(realpath -m "${2:?Usage: compile.sh SNAPSHOT NEW_BUILD}")
pdma_environment verification
: "${SVT_COMMON_HOME:?Configure the VIP common installation}"
: "${SVT_AMBA_HOME:?Configure the AXI VIP installation}"
: "${JVSUK_HOME:?Configure the jarvisuk checkout}"
: "${DESIGNWARE_HOME:?Configure the VIP installation root}"
export JVSUK_HOME SVT_COMMON_HOME SVT_AMBA_HOME
pdma_new_run "$build"
bash "$root/verification/dma/flow/prepare-jarvisuk.sh" "$JVSUK_HOME" "$build/jarvisuk"
export JVSUK_HOME="$build/jarvisuk"
(cd "$snapshot" && sha256sum --check --status snapshot.sha256)
cd "$root/verification/dma"
find tb env tests -type f -print0 | sort -z | xargs -0 sha256sum > "$build/testbench.sha256"
find "$JVSUK_HOME/src" "$JVSUK_HOME/include" "$JVSUK_HOME/interfaces" "$JVSUK_HOME/pkg" "$JVSUK_HOME/etc" \
    -type f -print0 | sort -z | xargs -0 sha256sum > "$build/jarvisuk.sha256"
find "$SVT_COMMON_HOME/sverilog" "$SVT_AMBA_HOME/sverilog" \
    -type f -print0 | sort -z | xargs -0 sha256sum > "$build/vip.sha256"
sha256sum "$root/tools/rtl-flow/environment.sh" "$root/verification/dma/flow/compile.sh" \
    "$snapshot/snapshot.sha256" > "$build/flow.sha256"
timeout "${PDMA_COMPILE_TIMEOUT:-1200}" "${PDMA_SIM_COMPILER:-vcs}" \
    -full64 -sverilog -ntb_opts uvm-1.2 -timescale=1ns/1ps -j4 \
    "+define+DESIGNWARE_INCDIR=$DESIGNWARE_HOME" \
    -cm line+cond+tgl+branch -cm_dir "$build/coverage.vdb" \
    -f "$snapshot/sources.f" -f tb/vip.f -top pdma_vip_top \
    -Mdir="$build/csrc" -o "$build/simv" -l "$build/compile.log" > "$build/console.log" 2>&1
sha256sum --check --status "$build/testbench.sha256"
sha256sum --check --status "$build/jarvisuk.sha256"
sha256sum --check --status "$build/vip.sha256"
sha256sum --check --status "$build/flow.sha256"
(cd "$snapshot" && sha256sum --check --status snapshot.sha256)
(cd "$build" && find simv simv.daidir -type f -print0 | sort -z | xargs -0 sha256sum > executable.sha256)
printf 'PDMA_VIP_COMPILE_COMPLETE\n'
