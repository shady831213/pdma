# Design-owned fixed profile; shared by synthesis and CDC/RDC adapters.
# Clock periods are ns. Interface members retain their emitted SV names.
return [dict create top pdma_rtl_DmaCopy clocks {
    reg_clk {period 2 reset reg_rst_n}
    core_clk {period 1 reset core_rst_n}
    bus_clk {period 1 reset bus_rst_n}
} reset_groups {
    cold {reg_rst_n core_rst_n bus_rst_n}
} memories {
    copy_buffer {
        instance engine/copy/buffers/u_ram provider tech_cell_generic_Ram1R1W
        kind Ram1R1W depth 8 width 512 clock core_clk
    }
} crossings {
    csr/crossing {provider common_cell cell DuplexMailbox source reg_clk destination core_clk max_delay 0.5}
    request_crossing {provider common_cell cell AsyncQueue source core_clk destination bus_clk max_delay 0.5 depth 8}
    response_crossing {provider common_cell cell AsyncQueue source bus_clk destination core_clk max_delay 0.5 depth 8}
} ports {
    reg_clk {
        input {
            cfg.awid 1 cfg.awaddr 32 cfg.awprot 3 cfg.awvalid 1
            cfg.wdata 32 cfg.wstrb 4 cfg.wvalid 1 cfg.bready 1
            cfg.arid 1 cfg.araddr 32 cfg.arprot 3 cfg.arvalid 1 cfg.rready 1
        }
        output {
            cfg.awready 1 cfg.wready 1 cfg.bid 1 cfg.bresp 2 cfg.bvalid 1
            cfg.arready 1 cfg.rid 1 cfg.rdata 32 cfg.rresp 2 cfg.rvalid 1
        }
    }
    core_clk {input {irq_ready 1} output {irq 1 fatal 1}}
    bus_clk {
        input {
            memory.awready 1 memory.wready 1 memory.bid 1 memory.bresp 2
            memory.buser 2 memory.bvalid 1 memory.arready 1 memory.rid 1
            memory.rdata 64 memory.rresp 2 memory.rlast 1 memory.ruser 2 memory.rvalid 1
        }
        output {
            awatop 6
            memory.awid 1 memory.awaddr 64 memory.awlen 8 memory.awsize 3
            memory.awburst 2 memory.awlock 1 memory.awcache 4 memory.awprot 3
            memory.awqos 4 memory.awregion 4 memory.awuser 32 memory.awvalid 1
            memory.wdata 64 memory.wstrb 8 memory.wlast 1 memory.wuser 1 memory.wvalid 1
            memory.bready 1 memory.arid 1 memory.araddr 64 memory.arlen 8 memory.arsize 3
            memory.arburst 2 memory.arlock 1 memory.arcache 4 memory.arprot 3
            memory.arqos 4 memory.arregion 4 memory.aruser 32 memory.arvalid 1 memory.rready 1
        }
    }
}]
