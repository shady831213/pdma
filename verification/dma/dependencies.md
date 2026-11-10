# Verification dependencies

The testbench uses a user-supplied Synopsys AXI VIP installation and UVM 1.2.
Proprietary sources are neither vendored nor redistributed. Configure
`SVT_COMMON_HOME`, `SVT_AMBA_HOME` and `DESIGNWARE_HOME` in the deployment environment.
The memory port currently uses AXI4 protocol checks because this RTL slice emits
only ordinary AXI5 reads/writes; the bench separately requires AWATOP to remain zero.
The register port uses the VIP's supplied `svt_axi_reg_adapter` with the same
master configuration and sequencer as the bus agent. PDMA generates its RAL
hierarchy from contract; it does not duplicate the vendor's protocol adapter.

[jarvisuk](https://github.com/shady831213/jarvisuk) is an external Apache-2.0
dependency pinned by [jarvisuk.lock.json](dependencies/jarvisuk.lock.json).
Set `JVSUK_HOME` to a Git checkout containing that revision. Preparation archives
the pinned upstream tree into the build directory without modifying the supplied
checkout or incorporating uncommitted edits. The allocator fix is upstream;
no local patch is applied. The manifest records dependency and preparation hashes.
See the [dependency preparation contract](dependencies/README.md).

The `pdma_clock_config` override corrects integer truncation in that revision's
initial phase calculation and supplies a full period: the transaction already
divides it into half periods. The clock driver and bounded jitter remain
jarvisuk's. Three independent clock groups use generated divider-one clocks.
Tests own the period/jitter/phase/reset configuration; `+clock_mode=fixed` selects
the fixed profile and the default random profile uses constrained values.
`get_freq()` and its bounds return Hz. An independent edge monitor checks actual
periods and synchronous reset release. Upstream reset completion pools use shared
short driver names, so the adapter independently awaits all three interfaces'
reset lifecycles before leaving the power-on `reset_phase`.
PDMA notifications have a ready/valid acceptance boundary, so the adapter sends
one transaction to jarvisuk's IRQ sequencer on each handshake. Its level-sensitive
interrupt monitor is not the same protocol and is not connected directly.
The adapter maps each channel's accepted notification to its configured vector,
and jarvisuk dispatches to a separate handler instance bound to that channel's
software object. Channel zero uses vector 17 to keep those identities distinct.
Separate checker instances also bind to their channels; the current DUT and
regression profile still have only one channel.
Channel reset does not reset the platform IRQ service or its handler registry.

The software channel observes completion by polling a Fence32/64 memory marker
after Copy. The task flow calls its checker before buffer release and waits for
reset drain when handling errors or cancellation;
passive coverage and performance subscribers do not own task resources.

The organization and VIP-native response pattern follow
[pcie_modeling](https://github.com/shady831213/pcie_modeling), without importing
its PCIe protocol model, deployment configuration or qualification claims.
