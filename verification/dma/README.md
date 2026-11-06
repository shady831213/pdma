# DMA RTL verification

The first phase owns a DMA-only DUT testbench, VCS/UVM/VIP integration and
DMA scenarios. CPU, peripheral RTL and complete subsystem scenarios are deferred.
`tests/minimal_copy.sv` directly exercises emitted RTL with continuously running
register/core/bus clocks, real MMIO, independent memory handshakes, Copy data
checks and reset during a delayed write. Compile it with the native generated
hardware filelist and top `MinimalCopyTest`; successful execution prints
`PDMA_MINIMAL_SV_PASS`. The bench uses generated package names and layout constants.
It is a bounded independent smoke oracle, not full protocol VIP coverage.
The reproducible simulator flows are in `flow/`. Deployment configuration and
raw logs stay outside version control.

The DMA bench is under `tb/`, with protocol VIP agents and jarvisuk memory,
clock and IRQ components assembled under `env/`. Software channel, MMIO and IRQ
handler objects under `env/software/` own software operations. Scenarios under
`tests/dut/` orchestrate task flows under `env/flow/`. Each task flow explicitly
invokes its checker under `env/checking/` before releasing buffers. Successful
tasks observe Fence completion; error/cancellation cleanup waits for reset drain.
Channels own persistent command/event rings; tasks separately allocate source,
destination and Fence buffers. Ring storage is released only at channel teardown.
Passive observations feed coverage and performance
collection under `env/coverage/`; they do not own task lifetime or decide completion.
Physical power-on reset belongs only in `reset_phase`; software reset remains a
scenario operation through the software objects. See
[requirements](requirements.md) for scenario traceability and
[dependencies](dependencies.md) for external inputs and adapter boundaries.
Measured checkpoints and their coverage gaps are in [results](results.md).

Software submits Copy followed by Fence32/64 and polls the real memory marker for
successful completion. Destination equality is a checker result, never a
completion signal. Fault, error and fatal results come from software-visible
registers and error events. Each channel has separate software, IRQ-handler and
checker instances. The current DUT implements only channel zero; its IRQ vector
is deliberately 17 to exercise vector-to-channel mapping without equating the
two identities. This organization does not claim multichannel DUT coverage.

After configuring the local environment described in
[the flow guide](../../tools/rtl-flow/flows.md):

```sh
bash tools/rtl-flow/snapshot.sh "$PDMA_RUN_ROOT/snapshot"
bash verification/dma/flow/compile.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/build"
bash verification/dma/flow/regression.sh "$PDMA_RUN_ROOT/build" "$PDMA_RUN_ROOT/regression"
bash verification/dma/flow/coverage.sh "$PDMA_RUN_ROOT/build" "$PDMA_RUN_ROOT/regression" "$PDMA_RUN_ROOT/coverage"
```

Run one case with `flow/run.sh BUILD NEW_RUN TEST SEED`. The 17 independently
selectable tests are listed in `flow/tests.list`. `PDMA_JOBS` bounds parallel runs
(default 4); `PDMA_TESTS` optionally selects a space-separated subset. Regression
output is separated by test and seed. Each output directory must be new; stale
output is rejected. The regression also runs deliberately corrupted
observations and requires the exact checker diagnostic: `data` / `TASK_DATA`,
`identity` and `unknown_identity` / `TASK_IDENTITY`, `missing_completion` / `TASK_TIMEOUT`, and
`duplicate_completion` / `TASK_DUPLICATE`. It does not count these negative tests
as DUT failures or include them in coverage.

`tb/contract_records.sv` is generated directly from the contract's schema through
the public export API. Typed byte images, field accessors, enum values, sizes and
alignments replace handwritten descriptor offsets. Regenerate with
`bash tools/rtl-flow/records.sh --write`; use `--check` for reproducibility.
Register addresses use the generated RTL layout packages. This is not a UVM RAL
generator. Rust codec fixtures independently check Copy/Fence images and Event
decoding in SystemVerilog.

The first decoder's bit slices are checked against the independently generated C
contract, including endpoint selectors and error-event fields. From the root:

```sh
cc -std=c11 -Wall -Wextra -Werror -fsyntax-only -Imodel/modeling/include verification/dma/tests/contract_layout.c
```

Observe AXI through VIP monitors and compare against independently derived
memory, completion, fault and reset expectations. Do not create a parallel bus
executor or require RTL arbitration to reproduce one Rust scheduling trace.
Firmware/Host execution and memory backing are testbench/platform components,
not synthesizable DMA behavior. This phase can drive ports directly; firmware or
CPU execution is not a prerequisite. Native block tests live under
[rtl/src/tests](../../rtl/src/tests/README.md).

Keep commercial runs and waves outside the repository. A report must identify sources,
parameters, tool versions, scenarios actually executed and their results;
zero tests, stale artifacts and timeouts do not establish a pass.
