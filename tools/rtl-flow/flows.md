# Reproducible RTL flows

The repository owns the executable build, simulation, regression, coverage,
synthesis and constraint code. The platform supplies installed tools, license
setup, proprietary libraries/VIP and output locations. A tool name or generic
option is public; machine deployment details are not.

Set `PDMA_ENV` to a trusted shell file outside the repository. It may export
configuration variables and define `pdma_setup()` accepting `simulation`,
`verification` or `synthesis`. The hook can load local modules and licenses.
The flow never dumps the environment. Do not put credentials in command-line
arguments or public examples. `PDMA_RUN_ROOT` below is a caller-selected output
directory; no particular machine path is required.

| Input | Purpose |
| --- | --- |
| `PDMA_SIM_COMPILER` | Simulator compiler executable; defaults to `vcs` |
| `PDMA_SYNTH_TOOL` | Synthesis executable; defaults to `dc_shell` |
| `PDMA_COVERAGE_TOOL` | Coverage executable; defaults to `urg` |
| `PDMA_TECH_CONFIG` | Private technology/library and timing-budget Tcl file |
| `SVT_COMMON_HOME`, `SVT_AMBA_HOME`, `DESIGNWARE_HOME` | Installed AXI VIP |
| `JVSUK_HOME` | jarvisuk source checkout |
| `PDMA_COMPILE_TIMEOUT`, `PDMA_RUN_TIMEOUT`, `PDMA_SYNTH_TIMEOUT`, `PDMA_COVERAGE_TIMEOUT` | Wall-clock limits in seconds |

Generate SystemVerilog on a machine that supports the pinned Veryl binary:

```sh
bash tools/rtl-flow/snapshot.sh "$PDMA_RUN_ROOT/snapshot"
bash verification/dma/flow/smoke.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/smoke"
bash impl/pdma/flow/synthesize.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/synthesis"
```

Run the commercial-tool stages on the configured execution host, locally or
through the platform's SSH/job launcher. Snapshot paths must be accessible there.
No execution-host identity is built into the flow. Each run has a fresh directory,
input hashes and output logs; timeouts and nonzero tool exits are failures.
Changing source requires a new snapshot/build. The directed smoke and UVM/VIP
regression are separate surfaces; neither substitutes for native unit tests.

`bash tools/rtl-flow/flow.test.sh` checks log failure detection and stale-output
rejection without requiring commercial tools. Temporary selftest fixtures are
removed on exit. Keep full tool output, waves and library manifests local;
publish only deliberately reviewed results without deployment information.
