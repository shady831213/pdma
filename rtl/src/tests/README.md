# Native block tests

Place Veryl native tests here, grouped by the hardware block they exercise.
Keep stimulus, clock/reset control, independent expectations and assertions visible
in each test. Shared support must not become a second hardware executor.

The seven executable tests cover:

| Test | Observable behavior |
| --- | --- |
| `DmaLayoutTest` | Schema offsets and parameterized admin/channel page boundary |
| `RegisterAccessTest` | 32/64-bit lane mapping, full/half configuration access, invalid strobes, permissions and reserved bits |
| `RegisterFrontendTest` | W-before-AW, read bypass, stalled request stability and held AXI responses |
| `RegisterCdcTest` | Independent clock advancement, startup and request/response backpressure without duplicate transfer |
| `RingRegistersTest` | Software/hardware cursor ownership, publication bounds, configuration freeze and cleanup |
| `DmaRegisterBankTest` | Three-channel isolation, live reset status, request versus cleanup completion |
| `DmaRegisterInterfaceTest` | Complete 32- and 64-bit AXI→CDC→owner paths, nonzero aperture base, read snapshot retention and exactly-once Resume under response backpressure |

Run both native CC and Cranelift backends with `VERYL_DUT_REUSE=0`, as shown in
[the RTL README](../../README.md). These fixtures drive lifecycle and IRQ status
inputs explicitly. They do not stand in for an implemented DMA engine, outstanding
memory drain or interrupt controller. Clock scheduling tests do not simulate
metastability or substitute for CDC/RDC analysis and physical timing checks.
