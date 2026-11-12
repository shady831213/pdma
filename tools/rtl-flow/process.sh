#!/usr/bin/env bash
# Owned-group teardown follows pcie_modeling's QTest runner contract.
pdma_execute() {
    local seconds=$1 output=$2 status=0
    shift 2
    setsid timeout --kill-after=5s "$seconds" "$@" > "$output" 2>&1 &
    pdma_process_pid=$!
    wait "$pdma_process_pid" || status=$?
    pdma_stop_process || status=2
    return "$status"
}

pdma_group_alive() {
    local group=$1 pgid state
    while read -r pgid state; do
        if [[ $pgid == "$group" && $state != Z* ]]; then return 0; fi
    done < <(ps -eo pgid=,stat=)
    return 1
}

pdma_stop_process() {
    [[ -n ${pdma_process_pid:-} ]] || return 0
    local deadline=$((SECONDS + 3))
    # A signal can arrive before setsid establishes the new process group.
    kill -TERM "$pdma_process_pid" 2>/dev/null || true
    kill -TERM -- "-$pdma_process_pid" 2>/dev/null || true
    while pdma_group_alive "$pdma_process_pid" && ((SECONDS < deadline)); do sleep 0.05; done
    if pdma_group_alive "$pdma_process_pid"; then
        kill -KILL -- "-$pdma_process_pid" 2>/dev/null || true
        deadline=$((SECONDS + 2))
        while pdma_group_alive "$pdma_process_pid" && ((SECONDS < deadline)); do sleep 0.05; done
        if pdma_group_alive "$pdma_process_pid"; then
            printf 'Owned process group did not exit before teardown deadline\n' >&2
            return 1
        fi
    fi
    wait "$pdma_process_pid" 2>/dev/null || true
    pdma_process_pid=
}
