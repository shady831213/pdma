# Design-owned elaboration profile; shared by synthesis and CDC/RDC adapters.
# Clock periods are ns. Interface members retain their emitted SV names.
if {[info exists ::env(PDMA_RTL_CONFIGURATION)]} {
    set configuration [source $::env(PDMA_RTL_CONFIGURATION)]
    if {[info exists ::env(PDMA_CHANNEL_COUNT)] && $::env(PDMA_CHANNEL_COUNT) ne [dict get $configuration channels]} {
        error "PDMA_PROFILE: channel override disagrees with immutable RTL snapshot"
    }
} else {
    set script [file normalize [file join [file dirname [info script]] ../../../rtl/flow/configuration.py]]
    if {[catch {exec python3 $script --tcl} text]} {error "PDMA_PROFILE: invalid DMA configuration"}
    set configuration $text
}
set channels [dict get $configuration channels]
set result [dict create top pdma_rtl_DmaConfigured clocks {
    reg_clk {period 2 reset reg_rst_n}
    core_clk {period 1 reset core_rst_n}
    bus_clk {period 1 reset bus_rst_n}
} reset_groups {
    cold {reg_rst_n core_rst_n bus_rst_n}
} memories {
    copy_buffer {
        instance dma/commands/execution/copy/buffers/u_ram provider tech_cell_generic_Ram1R1W
        kind Ram1R1W clock core_clk
    }
} crossings {
    dma/control/csr/crossing {provider common_cell cell DuplexMailbox source reg_clk destination core_clk max_delay 0.5}
    dma/transport/fetch_crossing {provider common_cell cell AsyncQueue source core_clk destination bus_clk max_delay 0.5}
    dma/transport/request_crossing {provider common_cell cell AsyncQueue source core_clk destination bus_clk max_delay 0.5}
    dma/transport/response_crossing {provider common_cell cell AsyncQueue source bus_clk destination core_clk max_delay 0.5}
} ports {
    reg_clk {
        input {
            cfg.awid 1 cfg.awaddr 32 cfg.awprot 3 cfg.awvalid 1
            cfg.wvalid 1 cfg.bready 1
            cfg.arid 1 cfg.araddr 32 cfg.arprot 3 cfg.arvalid 1 cfg.rready 1
        }
        output {
            cfg.awready 1 cfg.wready 1 cfg.bid 1 cfg.bresp 2 cfg.bvalid 1
            cfg.arready 1 cfg.rid 1 cfg.rresp 2 cfg.rvalid 1
        }
    }
    bus_clk {
        input {
            memory.awready 1 memory.wready 1 memory.bid 1 memory.bresp 2
            memory.buser 2 memory.bvalid 1 memory.arready 1 memory.rid 1
            memory.rresp 2 memory.rlast 1 memory.ruser 2 memory.rvalid 1
        }
        output {
            memory.awid 1 memory.awaddr 64 memory.awlen 8 memory.awsize 3
            memory.awburst 2 memory.awlock 1 memory.awcache 4 memory.awprot 3
            memory.awqos 4 memory.awregion 4 memory.awuser 35 memory.awvalid 1
            memory.wlast 1 memory.wuser 1 memory.wvalid 1
            memory.bready 1 memory.arid 1 memory.araddr 64 memory.arlen 8 memory.arsize 3
            memory.arburst 2 memory.arlock 1 memory.arcache 4 memory.arprot 3
            memory.arqos 4 memory.arregion 4 memory.aruser 35 memory.arvalid 1 memory.rready 1
        }
    }
}]

# Add only the profile-dependent dimensions here; the protocol constants above
# have no competing defaults for configurable ports or storage.
dict set result configuration $configuration
dict set result memories copy_buffer depth [dict get $configuration rows]
dict set result memories copy_buffer width [dict get $configuration row_bits]
dict set result crossings dma/transport/fetch_crossing depth [dict get $configuration request_depth]
dict set result crossings dma/transport/request_crossing depth [dict get $configuration request_depth]
dict set result crossings dma/transport/response_crossing depth [dict get $configuration receipt_depth]
dict set result ports reg_clk input cfg.wdata [dict get $configuration register_bits]
dict set result ports reg_clk input cfg.wstrb [dict get $configuration register_bytes]
dict set result ports reg_clk output cfg.rdata [dict get $configuration register_bits]
if {[dict get $configuration memory_axi_version] == 5} {
    dict set result ports bus_clk output awatop 6
}
foreach port {memory.rid memory.bid} {dict set result ports bus_clk input $port [dict get $configuration id_bits]}
foreach port {memory.arid memory.awid} {dict set result ports bus_clk output $port [dict get $configuration id_bits]}
dict set result ports bus_clk input memory.rdata [dict get $configuration memory_bits]
dict set result ports bus_clk output memory.wdata [dict get $configuration memory_bits]
dict set result ports bus_clk output memory.wstrb [dict get $configuration memory_bytes]
dict set result ports core_clk input [dict create irq_ready $channels fatal_irq_ready 1]
dict set result ports core_clk output [dict create irq $channels fatal 1 fatal_irq 1]
return $result
