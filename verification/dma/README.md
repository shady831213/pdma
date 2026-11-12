# DMA RTL verification

The first phase owns a DMA-only DUT testbench, VCS/UVM/VIP integration and
DMA scenarios. CPU, peripheral RTL and complete subsystem scenarios are deferred.
`tests/minimal_copy.sv` directly exercises emitted RTL with continuously running
register/core/bus clocks, real MMIO, independent memory handshakes, Copy/Fence64
completion and reset during a delayed write. It constructs descriptors with the
generated record package. Compile that package and the native generated
hardware filelist with top `MinimalCopyTest`; successful execution prints
`PDMA_MINIMAL_SV_PASS`. The bench uses generated package names and layout constants.
It is a bounded independent smoke oracle, not full protocol VIP coverage.
Normal completion comes from Fence storage. Reset checks retained W-before-AW
work, busy while B is withheld, no successor Fence, no traffic after reset finishes,
and a fresh Copy/Fence afterwards. It does not assume exactly one outstanding burst
or use a write counter as a software completion signal.
The reproducible simulator flows are in `flow/`. Deployment configuration and
raw logs stay outside version control.

The DMA bench is under `tb/`, with protocol VIP agents and jarvisuk memory,
clock and IRQ components assembled under `env/`. Software channel and IRQ
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

`env/package.sv` owns reusable environment classes and includes no scenarios.
`tests/package.sv` owns DUT scenarios and TB selftests; `tb/top.sv` imports both.
Generated contract packages live under `tb/generated/`, assembly and checker binds
remain under `tb/`, and DMA lifecycle assertions live under `checkers/`.
The software task owns payload/Fence allocation and descriptor construction;
observed results and passive coverage observations have their own data types.
The root `tests/*.sv` files are standalone oracles/instrumentation, not UVM
scenario includes; `tb/vip.f` explicitly selects the environment and scenario packages.

`tb/bindings.sv` binds package-owned passive checkers to real mailbox and FIFO ports to check retained
ownership, payload, no duplicate delivery and backpressure stability. It adds no
clock-period polling or DUT scheduling. Definitions are loaded from `common_cell`'s
declared snapshot resources, never from an independently selected source checkout.
`tests/cdc_custody.sv` qualifies these actual bindings, including corrupted
data, duplicate delivery, premature overwrite and broken hold negative cases:

```sh
bash verification/dma/flow/cdc-custody.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/cdc-custody"
bash verification/dma/flow/cdc-custody.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/queue-custody" queue
```

Some simulators return zero for `$fatal`; the negative gate requires the exact
fatal diagnostic and absence of the success marker. Timeout and signal exits do
not qualify. This monitor checks digital protocol invariants; static CDC/RDC and
physical timing/MTBF obligations remain separate.

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

Run one case with `flow/run.sh BUILD NEW_RUN TEST SEED`. The 24 independently
selectable tests are listed in `flow/tests.list`. `PDMA_JOBS` bounds parallel runs
(default 4); `PDMA_TESTS` optionally selects a space-separated subset.
`PDMA_CLOCK_MODE=random|fixed|target` selects the ordinary clock profile. The four
throughput tests explicitly select constant-period target or half-core clocks,
zero responder delay, sustained READY and sufficient VIP outstanding capacity.
Their passive callbacks measure individual payload beats and descriptor-to-Fence
latency. The batch test prepublishes four Copy/Fence pairs while disabled, then
enables once, so software cannot starve the measured command boundaries.

For a separate simulation-only latency stress snapshot:

```sh
bash verification/dma/flow/sync-latency.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/latency-snapshot"
```

Compile/run that snapshot using the same verification flows. It adds independently
seeded 0/1 destination-cycle sample delay per synchronizer instance, preserving
coherent Gray vectors. It is a digital latency test, not metastability or MTBF
qualification; implementation flows reject its simulation-only marker.
Ordinary zero-delay tests require zero payload gaps. The instrumented build still
requires uninterrupted reads, and bounds **total** write gaps by the configured
sample-delay variation plus the one-cycle fair receipt-arbitration delay, with
receiver-clock rounding. This is three bus cycles at equal clocks and five with
core at half bus frequency, not a per-burst allowance.
It permits a bounded initial elastic-buffer fill transient, never periodic loss;
gap positions, read-to-write handoff range and full-task latency are all reported.

The VIP memory responder returns early W-first notifications in zero time, as
required by its API. Address-dependent write permissions and injected failure
are applied through the actual AW-ended callback before native memory update;
the placeholder address in an early W notification is never treated as real.
The native VIP continues to own transaction assembly and memory effects.

`PDMA_SUITE=dut` is the default. Clock period/jitter selfchecks are not instantiated
in DUT runs; clock/reset integration lives in `env/clocks.svh`. AXI protocol checks,
DUT assertions and the IRQ handshake monitor remain active. The environment uses
`configure_phase` for handler registration and `main_phase` for the IRQ service;
power-on reset stays in `reset_phase`.

The separate `PDMA_SUITE=tb-selftest` suite contains five independently selectable
cases: clock/reset-stimulus qualification, IRQ wait success isolation, timeout cleanup,
concurrent waits and reset-thread isolation. Its minimal fixture does not construct
DMA software flows or functional coverage collectors. These results are TB
qualification, and `flow/coverage.sh` rejects them as DUT coverage inputs.
Both suites use the same bounded parallel runner:

```sh
PDMA_SUITE=tb-selftest bash verification/dma/flow/regression.sh "$PDMA_RUN_ROOT/build" "$PDMA_RUN_ROOT/tb-selftest" 1 7
```

Regression
output is separated by test and seed. Each output directory must be new; stale
output is rejected. The regression also runs deliberately corrupted
observations and requires the exact checker diagnostic: `data` / `TASK_DATA`,
`identity` and `unknown_identity` / `TASK_IDENTITY`, `missing_completion` / `TASK_TIMEOUT`, and
`duplicate_completion` / `TASK_DUPLICATE`, early Fence publication during an IRQ
error / `TASK_COMPLETION`, and understated or impossible fractional error progress /
`TASK_ERROR_INFO`. The latter cases run against directed error tests. It does not count these negative tests
as DUT failures or include them in coverage.
Timeouts and signal exits remain failures even if a negative run printed its
expected diagnostic. Coverage reports are emitted as text summaries with all
collected metrics; only successful positive runs enter the merge. Raw databases
remain available for detailed reporting. The installed report tool crashes in
the detailed-report path for this suite, while summary reporting succeeds with
the identical full input set; this is not a metric or bin exclusion.

`tb/generated/contract_records.sv` is generated directly from the contract's schema through
the public export API. Typed byte images, field accessors, enum values, sizes and
alignments replace handwritten descriptor offsets. Regenerate with
`bash tools/rtl-flow/records.sh --write`; use `--check` for reproducibility.
Rust codec fixtures independently check Copy/Fence images and Event decoding in
SystemVerilog.

`tb/generated/register_model.sv` is the generated UVM register model. Regenerate with
`bash tools/rtl-flow/registers.sh --write` and verify with `--check`. Its generator
selects the DMA roots in contract's `src/bin/uvm_regs`; its renderer is the optional
schema `UvmRegisterBackend`. SV record rendering likewise uses schema's builtin
`SvRecordBackend`. The contract wrappers own package/import/file assembly only.
The backends are supplied by registry schema 0.3.0, with no local override;
see the model [schema review record](../../model/modeling/docs/schema-export-review.md).

`tb/generated/register_layout.sv` is the separate plain-SV register metadata package set,
with no UVM dependency. Generate/check it with `bash tools/rtl-flow/sv-layout.sh
--write` / `--check`. It exposes the same symbolic layouts and fixed register facts
as the Veryl packages for consumers that use SV directly. It contains no register
storage or bus implementation.

The model preserves block hierarchy and parameterized banks, for example
`registers.channels_block[i].command_block.base_reg`. Register fields have
`_field` names; register and block handles use `_reg` and `_block` to avoid language
keywords and inherited UVM methods. Build with `DmaRegisters.build(channel_count,
bus_bytes)`, then configure the root base, lock the model and bind its map.
The environment uses the VIP's native `svt_axi_reg_adapter`; native UVM maps
split 64-bit registers into 32-bit bus accesses. The old address-based sequence
and manual split-write helper are removed.

Software uses explicit frontdoor reads/writes and checks their status. Hardware
reset, Resume and IRQ side effects occur in the DUT, not through RAL mirror
updates or `uvm_reg_block.reset()`. Schema metadata does not identify all hardware
updates or W1C behavior: fields are conservatively volatile, mirror comparison
is disabled, and no side-effect policy is inferred from descriptions. Zero and
Constant resets are known; PostReset is not assigned a known HARD reset value.
This initial backend supports the DMA's little-endian register graph. Big-endian
registers, partial writable masks within a field and permission-only projected
views without a reusable layout plan fail explicitly rather than being guessed.
Application build parameters must still satisfy the schema's layout constraints.
Register descriptions and field enum constants now use shared value callbacks.
Enums appear as field-qualified class constants; collisions after name formatting
are rejected. Registers without declared fields use a whole-width RAL field as a
target representation choice. SV record unions retain byte-image accessors while
inline alternatives contribute case-qualified enum and bit metadata.

`flow/schema.sh NEW_RUN TOP PASS_MARKER SOURCE...` compiles and runs a standalone
generated-package oracle using the same deployment configuration. For the RAL
oracle, select top `pdma_uvm_regs_metadata`, marker `RAL_METADATA_PASS`, and sources
`tb/generated/register_model.sv` and the contract's `tests/uvm_regs_metadata.sv`. It checks
enum constants, relocated maps, 32/64-bit buses and parameterized banks. The SV
schema's `sv_records` test can retain its standalone oracle with
`SCHEMA_SV_INLINE_FIXTURE`; its top and marker are `inline_union_metadata` and
`INLINE_UNION_PASS`. The `sv_register_layout` and `uvm_exports` tests provide
independent parameter/width oracles through `SCHEMA_SV_REGISTER_FIXTURE`,
`SCHEMA_UVM_ADDRESS_FIXTURE` and `SCHEMA_UVM_PARAMETER_FIXTURE`.
Keep generated fixtures and native run artifacts untracked.

The configuration separates this DUT's fixed bus widths and AXI burst boundary
from platform heap placement, ring capacities, optional explicit ring addresses
and software polling limits. The event-boundary test places one entry across a
4 KiB boundary and checks its decoded identity/code/progress and the neighboring
slot. Coverage uses allocated addresses, rather than requested placement flags,
to classify alignment and boundary crossings.

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
