# PDMA system RTL verification

The first phase owns a DMA-only DUT testbench, VCS/UVM/VIP integration and
DMA scenarios. CPU, peripheral RTL and complete subsystem scenarios are deferred.
`tests/minimal_copy.sv` directly exercises emitted RTL with continuously running
register/core/bus clocks, real MMIO, independent memory handshakes, Copy data
checks and reset during a delayed write. Compile it with the native generated
hardware filelist and top `MinimalCopyTest`; successful execution prints
`PDMA_MINIMAL_SV_PASS`. The bench uses generated package names and layout constants.
It is a bounded independent smoke oracle, not full protocol VIP coverage.
Commercial simulator invocation, environment and logs remain outside the repository.

The first decoder's bit slices are checked against the independently generated C
contract, including endpoint selectors and error-event fields. From the root:

```sh
cc -std=c11 -Wall -Wextra -Werror -fsyntax-only -Imodel/modeling/include verification/pdma/tests/contract_layout.c
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
