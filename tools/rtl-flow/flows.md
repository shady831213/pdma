# Reproducible RTL flows

The repository owns the executable build, simulation, regression, coverage,
synthesis and constraint code. The platform supplies installed tools, license
setup, proprietary libraries/VIP and output locations. A tool name or generic
option is public; machine deployment details are not.

The implementation baseline requires Genus at the approved per-domain
frequencies and a non-ULVT library policy. CDC and RDC require VC SpyGlass.
The adapters and their qualification entries are described in
[implementation checks](../../impl/dma/README.md). Reusable launcher,
constraint and report-check code belongs here or in the design flow directories;
only deployment and proprietary inputs remain private.

Set `PDMA_ENV` to a trusted shell file outside the repository. It may export
configuration variables and define `pdma_setup()` accepting `simulation`,
`verification`, `synthesis` or `static`. The hook can load local modules and licenses.
The flow never dumps the environment. Do not put credentials in command-line
arguments or public examples. `PDMA_RUN_ROOT` below is a caller-selected output
directory; no particular machine path is required.

| Input | Purpose |
| --- | --- |
| `PDMA_SIM_COMPILER` | Simulator compiler executable; defaults to `vcs` |
| `PDMA_GENUS_TOOL` | Genus executable; defaults to `genus` |
| `PDMA_STATIC_TOOL` | VC SpyGlass executable; defaults to `vc_static_shell` |
| `PDMA_COVERAGE_TOOL` | Coverage executable; defaults to `urg` |
| `PDMA_TECH_CONFIG` | Private technology/library and timing-budget Tcl file |
| `PDMA_DESIGN_PROFILE` | Optional top/clock/port profile; defaults to the DMA Copy slice |
| `PDMA_STATIC_REVIEW` | Optional explicit source-bound CDC/RDC review policy; default raw findings remain failures |
| `PDMA_SYNTH_STAGE` | Genus `prepare` or default `map`; preparation alone is not synthesis qualification |
| `PDMA_MEMORY_MODE` | Default `control-budget` binds inventoried RAM boundaries; `generic` maps behavioral arrays to standard cells |
| `SVT_COMMON_HOME`, `SVT_AMBA_HOME`, `DESIGNWARE_HOME` | Installed AXI VIP |
| `JVSUK_HOME` | jarvisuk source checkout |
| `PDMA_COMPILE_TIMEOUT`, `PDMA_RUN_TIMEOUT`, `PDMA_SYNTH_TIMEOUT`, `PDMA_STATIC_TIMEOUT`, `PDMA_COVERAGE_TIMEOUT` | Wall-clock limits in seconds |

Generate SystemVerilog on a machine that supports the pinned Veryl binary:

```sh
bash tools/rtl-flow/snapshot.sh "$PDMA_RUN_ROOT/snapshot"
bash verification/dma/flow/smoke.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/smoke"
bash impl/dma/flow/genus.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/synthesis"
bash impl/dma/flow/crossings.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/cdc" cdc
bash impl/dma/flow/crossings.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/rdc" rdc
```

Run the commercial-tool stages on the configured execution host, locally or
through the platform's SSH/job launcher. Snapshot paths must be accessible there.
No execution-host identity is built into the flow. Each run has a fresh directory,
input hashes and output logs; timeouts and nonzero tool exits are failures.
Changing source requires a new snapshot/build. The directed smoke and UVM/VIP
regression are separate surfaces; neither substitutes for native unit tests.

The `impl/dma/flow` entries select the DMA profile and delegate to generic adapters
under `tools/rtl-flow/implementation`. Direct callers of those adapters must set
`PDMA_DESIGN_PROFILE`. Package contracts come from the supplied snapshot, never
from a guessed source-tree/cache path. See [package resources](ip/README.md).
Compile, simulation, coverage and implementation commands share owned process-group
execution and teardown; cancellation stops descendants before returning. Log checks
reject native failures even if a success marker was printed earlier.

`bash tools/rtl-flow/tests/environment_test.sh` checks log failure detection and stale-output
rejection without requiring commercial tools. Temporary selftest fixtures are
removed on exit. Keep full tool output, waves and library manifests local;
publish only deliberately reviewed results without deployment information.
