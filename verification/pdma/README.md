# PDMA system RTL verification

This directory will own the assembled DUT testbench, VCS/UVM/VIP integration and
system scenarios. Add `tb`, `tests` and any model peer support when implemented;
there are no runnable testbenches or flow scripts yet.

Observe AXI through VIP monitors and compare against independently derived
memory, completion, fault and reset expectations. Do not create a parallel bus
executor or require RTL arbitration to reproduce one Rust scheduling trace.
Firmware/Host execution and memory backing are testbench/platform components,
not synthesizable DMA behavior. Native block tests live under
[rtl/src/tests](../../rtl/src/tests/README.md).

Keep temporary runs and waves in `.run/`. A report must identify sources,
parameters, tool versions, scenarios actually executed and their results;
zero tests, stale artifacts and timeouts do not establish a pass.
