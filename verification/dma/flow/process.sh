#!/usr/bin/env bash
# Owned-group teardown follows pcie_modeling's QTest runner contract.
pdma_group_alive() {
    local group=$1 pgid state
    while read -r pgid state; do
        if [[ $pgid == "$group" && $state != Z* ]]; then return 0; fi
    done < <(ps -eo pgid=,stat=)
    return 1
}

pdma_stop_simulation() {
    [[ -n ${simulation_pid:-} ]] || return 0
    local deadline=$((SECONDS + 3))
    # A signal can arrive before setsid establishes the new process group.
    kill -TERM "$simulation_pid" 2>/dev/null || true
    kill -TERM -- "-$simulation_pid" 2>/dev/null || true
    while pdma_group_alive "$simulation_pid" && ((SECONDS < deadline)); do sleep 0.05; done
    if pdma_group_alive "$simulation_pid"; then
        kill -KILL -- "-$simulation_pid" 2>/dev/null || true
        deadline=$((SECONDS + 2))
        while pdma_group_alive "$simulation_pid" && ((SECONDS < deadline)); do sleep 0.05; done
        if pdma_group_alive "$simulation_pid"; then
            printf 'Simulation process group did not exit before teardown deadline\n' >&2
            return 1
        fi
    fi
    wait "$simulation_pid" 2>/dev/null || true
    simulation_pid=
}
