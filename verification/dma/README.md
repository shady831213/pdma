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

Run one case with `flow/run.sh BUILD NEW_RUN TEST SEED`. The 19 independently
selectable tests are listed in `flow/tests.list`. `PDMA_JOBS` bounds parallel runs
(default 4); `PDMA_TESTS` optionally selects a space-separated subset.
`PDMA_CLOCK_MODE=random|fixed` selects the clock profile for the entire batch.
Regression
output is separated by test and seed. Each output directory must be new; stale
output is rejected. The regression also runs deliberately corrupted
observations and requires the exact checker diagnostic: `data` / `TASK_DATA`,
`identity` and `unknown_identity` / `TASK_IDENTITY`, `missing_completion` / `TASK_TIMEOUT`, and
`duplicate_completion` / `TASK_DUPLICATE`, early Fence publication during an IRQ
error / `TASK_COMPLETION`, and understated error progress / `TASK_ERROR_INFO`.
The latter two run against the directed error test. It does not count these negative tests
as DUT failures or include them in coverage.
Timeouts and signal exits remain failures even if a negative run printed its
expected diagnostic. Coverage reports are emitted as text summaries with all
collected metrics; only successful positive runs enter the merge. Raw databases
remain available for detailed reporting. The installed report tool crashes in
the detailed-report path for this suite, while summary reporting succeeds with
the identical full input set; this is not a metric or bin exclusion.

`tb/contract_records.sv` is generated directly from the contract's schema through
the public export API. Typed byte images, field accessors, enum values, sizes and
alignments replace handwritten descriptor offsets. Regenerate with
`bash tools/rtl-flow/records.sh --write`; use `--check` for reproducibility.
Rust codec fixtures independently check Copy/Fence images and Event decoding in
SystemVerilog.

`tb/register_model.sv` is the generated UVM register model. Regenerate with
`bash tools/rtl-flow/registers.sh --write` and verify with `--check`. Its generator
lives in contract's `src/bin/uvm_regs`, alongside the separate SV record generator;
neither backend has been moved into schema or published. During the authorized
unpublished render-API review, these generators use the explicit local schema
workspace override. They consume schema analysis and expression-rendering APIs
without parsing generated C/Veryl; see the model
[schema review record](../../model/modeling/docs/schema-export-review.md).

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
`tb/register_model.sv` and the contract's `tests/uvm_regs_metadata.sv`. It checks
enum constants, relocated maps, 32/64-bit buses and parameterized banks. The SV
generator's inline-union test can retain its standalone oracle with
`PDMA_SV_INLINE_FIXTURE`; its top and marker are `inline_union_metadata` and
`INLINE_UNION_PASS`. Keep run directories outside the repository.

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
