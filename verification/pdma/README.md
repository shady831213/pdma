# PDMA system RTL verification

The first phase owns a DMA-only DUT testbench, VCS/UVM/VIP integration and
DMA scenarios. CPU, peripheral RTL and complete subsystem scenarios are deferred.
Add `tb`, `tests` and any model peer support when implemented;
there are no runnable testbenches or flow scripts yet.

Observe AXI through VIP monitors and compare against independently derived
memory, completion, fault and reset expectations. Do not create a parallel bus
executor or require RTL arbitration to reproduce one Rust scheduling trace.
Firmware/Host execution and memory backing are testbench/platform components,
not synthesizable DMA behavior. This phase can drive ports directly; firmware or
CPU execution is not a prerequisite. Native block tests live under
[rtl/src/tests](../../rtl/src/tests/README.md).

Keep temporary runs and waves in `.run/`. A report must identify sources,
parameters, tool versions, scenarios actually executed and their results;
zero tests, stale artifacts and timeouts do not establish a pass.
