# RAM accounting and hierarchy optimization qualification

Date: 2026-11-13. Functional RTL baseline: `cc7ce4ce`.
The [RAM accounting contract](memory-budget.md) describes the implementation;
this record describes the measured scope and its limits.

## Change and ownership

The Copy profile explicitly lists its 8x512-bit, core-clock 1R1W provider.
Genus binds that leaf to a generated pin/capacitance library. Functional simulation
continues to use the behavioral provider. Standard-library FIFO arrays and all
other storage remain synthesized logic. The emitter/binder/audits are shared
flow code; instance, geometry and clock selection belong to the design profile.

Ordinary RTL now enables automatic ungrouping and boundary optimization.
Retiming remains off. RAM leaves have independent preservation; the 40 mapped
synchronizer registers retain their protection. Package-owned CDC queries use
native RTL-origin names after ungroup, without guessing new names, keeping stale
collections or preserving entire modules to satisfy a selector.

## Qualification

- Baseline inspection confirmed zero RAM blackboxes and disabled ungrouping.
  The new expectation failed on that database before implementation.
- Twelve serializer/report-resource tests and 23 invalid-profile cases pass; nine package-resource
  tests pass. Native tests cover equal-shaped RAMs with separate identities and
  one-bit/one-row RAMs. Each configuration passes its positive checks and rejects
  11 invalid clock-coverage, delay, load, geometry, ownership and hierarchy cases.
  Generic mode also maps the small fixture successfully without opaque leaves.
- The real CDC integration fixture passes after hierarchy optimization: exact
  neighboring-instance isolation, 40 synchronizer registers, local timing,
  missing scopes, wrong depth, impossible budgets and masked paths are checked.
- Native CDC/RDC fields match the prior snapshot exactly. The stale review is
  rejected before rebinding; review rules are unchanged. Final results are
  **CDC 40/40/0 and RDC 1363/1363/0** raw/reviewed/unresolved findings.
- Generated functional SystemVerilog is byte-identical to the prior qualified
  snapshot. No model or RTL behavior changed; existing functional/throughput
  evidence retains its original identity. This is not a newly run UVM regression.
- The current serializer reproduces the actual mapped RAM library byte-for-byte.
  Current helper code independently re-audits the immutable mapped database with
  its frozen profile, technology settings and package resources. RAM identity,
  loads, boundary delays, library policy and CDC/timing checks all pass.

## Mapped result

Target and technology selection remain unchanged: 12nm/Genus, no ULVT,
core/bus 1GHz and register clock 500MHz, with three asynchronous domains.

| Measurement | Result |
| --- | ---: |
| Worst mapped setup slack | 0ps |
| Standard-cell instances | 56,552 |
| Opaque RAM leaves | 1 |
| Sequential instances reported by the tool | 20,571 |
| Mapped cell area, excluding physical SRAM | 27,681.546 library units |
| Excluded logical RAM capacity | 4096 bits / 512B |
| RAM read-output boundary paths | 512; minimum accounting slack 919.3ps |
| RAM dynamic input boundary paths | 520; minimum accounting slack 199.0ps |
| Crossing groups / held-payload checks / local chains | 8 / 4 / 8, all passing |
| Minimum crossing / local-stage slack | 207.6ps / 820.6ps |

The mapped standard-cell identities pass the non-ULVT policy; the accounting leaf
does not qualify a future SRAM's technology or threshold-voltage choice. Tightening the
restored database's core period from 1ns to 0.9ns produces -100ps worst slack and
is correctly rejected. The original producer database remains unchanged.

The old 32,267.907-area result included behavioral RAM implemented with registers
and retained ordinary module boundaries. It is a different measurement scope and
optimization policy. Its difference from the new result is not an equivalent
area saving, and the missing SRAM cost must not be treated as zero.

This remains pre-layout logic and boundary-budget qualification. A real SRAM must
fit the port, latency, mask, collision and timing contracts. Physical macro
area/power/timing, hold, routed parasitics, reset recovery/removal, MTBF and workload
power are not qualified here. The synthesis accounting model does not hide storage
from the separate behavioral-RTL CDC/RDC analysis or add structural waivers.
