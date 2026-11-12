module pdma_vip_dut(svt_axi_if host_bus, svt_axi_if memory_bus, pdma_control_if control);
    __std___axi4_lite_if____std___axi4_lite_pkg__32__4__1 cfg();
    __std___axi4_if____std___axi4_pkg__64__8__1__32__1__2__32__2 memory();
    pdma_rtl_DmaCopy dut(
        .reg_clk(control.reg_clk), .core_clk(control.core_clk), .bus_clk(control.bus_clk),
        .reg_rst_n(control.reg_rst_n), .core_rst_n(control.core_rst_n), .bus_rst_n(control.bus_rst_n),
        .cfg(cfg), .memory(memory), .awatop(control.awatop),
        .irq(control.irq), .irq_ready(control.irq_ready), .fatal(control.fatal)
    );
    pdma_lifecycle_assertions lifecycle(
        .clk(control.core_clk),.rst_n(control.core_rst_n),
        .request(dut.requests.valid),.ready(dut.requests.ready),
        .returned(dut.requests.returned),.accepted(dut.requests.accepted),
        .request_slot(dut.requests.request.slot),.receipt_slot(dut.requests.receipt.slot),
        .closing(dut.engine.shutdown),.clear(dut.clear),
        .enabled(dut.configurations[0].enable),.has_commands(dut.commands[0].producer!=dut.commands[0].consumer),
        .irq(control.irq),.irq_ready(control.irq_ready),
        .irq_source(control.fatal || dut.statuses[0].error || dut.statuses[0].fatal!=0 ||
            dut.statuses[0].trap || dut.events[0].producer!=dut.events[0].consumer),
        .parked(dut.statuses[0].faulted),.resume_request(dut.actions[0].resume_request),
        .parked_state({dut.engine.descriptor,dut.engine.copy.next_source,dut.engine.copy.next_destination,
            dut.engine.copy.remaining,dut.engine.copy.committed,dut.engine.fence_data})
    );
    assign cfg.awaddr=host_bus.master_if[0].awaddr;
    assign cfg.awprot=host_bus.master_if[0].awprot;
    assign cfg.awvalid=host_bus.master_if[0].awvalid;
    assign host_bus.master_if[0].awready=cfg.awready;
    assign cfg.wdata=host_bus.master_if[0].wdata;
    assign cfg.wstrb=host_bus.master_if[0].wstrb;
    assign cfg.wvalid=host_bus.master_if[0].wvalid;
    assign host_bus.master_if[0].wready=cfg.wready;
    assign host_bus.master_if[0].bresp=cfg.bresp;
    assign host_bus.master_if[0].bvalid=cfg.bvalid;
    assign cfg.bready=host_bus.master_if[0].bready;
    assign cfg.araddr=host_bus.master_if[0].araddr;
    assign cfg.arprot=host_bus.master_if[0].arprot;
    assign cfg.arvalid=host_bus.master_if[0].arvalid;
    assign host_bus.master_if[0].arready=cfg.arready;
    assign host_bus.master_if[0].rdata=cfg.rdata;
    assign host_bus.master_if[0].rresp=cfg.rresp;
    assign host_bus.master_if[0].rvalid=cfg.rvalid;
    assign cfg.rready=host_bus.master_if[0].rready;
    assign memory_bus.slave_if[0].awaddr=memory.awaddr;
    assign memory_bus.slave_if[0].awvalid=memory.awvalid;
    assign memory_bus.slave_if[0].awid=memory.awid;
    assign memory_bus.slave_if[0].awlen=memory.awlen;
    assign memory_bus.slave_if[0].awsize=memory.awsize;
    assign memory_bus.slave_if[0].awburst=memory.awburst;
    assign memory_bus.slave_if[0].awlock=memory.awlock;
    assign memory_bus.slave_if[0].awcache=memory.awcache;
    assign memory_bus.slave_if[0].awprot=memory.awprot;
    assign memory_bus.slave_if[0].awqos=memory.awqos;
    assign memory_bus.slave_if[0].awregion=memory.awregion;
    assign memory_bus.slave_if[0].awuser=memory.awuser;
    assign memory.awready=memory_bus.slave_if[0].awready;
    assign memory_bus.slave_if[0].wdata=memory.wdata;
    assign memory_bus.slave_if[0].wstrb=memory.wstrb;
    assign memory_bus.slave_if[0].wlast=memory.wlast;
    assign memory_bus.slave_if[0].wuser=memory.wuser;
    assign memory_bus.slave_if[0].wvalid=memory.wvalid;
    assign memory.wready=memory_bus.slave_if[0].wready;
    assign memory.bvalid=memory_bus.slave_if[0].bvalid;
    assign memory.bid=memory_bus.slave_if[0].bid;
    assign memory.bresp=__std_axi4_config::resp_variants'(memory_bus.slave_if[0].bresp);
    assign memory.buser=memory_bus.slave_if[0].buser;
    assign memory_bus.slave_if[0].bready=memory.bready;
    assign memory_bus.slave_if[0].araddr=memory.araddr;
    assign memory_bus.slave_if[0].arvalid=memory.arvalid;
    assign memory_bus.slave_if[0].arid=memory.arid;
    assign memory_bus.slave_if[0].arlen=memory.arlen;
    assign memory_bus.slave_if[0].arsize=memory.arsize;
    assign memory_bus.slave_if[0].arburst=memory.arburst;
    assign memory_bus.slave_if[0].arlock=memory.arlock;
    assign memory_bus.slave_if[0].arcache=memory.arcache;
    assign memory_bus.slave_if[0].arprot=memory.arprot;
    assign memory_bus.slave_if[0].arqos=memory.arqos;
    assign memory_bus.slave_if[0].arregion=memory.arregion;
    assign memory_bus.slave_if[0].aruser=memory.aruser;
    assign memory.arready=memory_bus.slave_if[0].arready;
    assign memory.rvalid=memory_bus.slave_if[0].rvalid;
    assign memory.rid=memory_bus.slave_if[0].rid;
    assign memory.rdata=memory_bus.slave_if[0].rdata;
    assign memory.rresp=__std_axi4_config::resp_variants'(memory_bus.slave_if[0].rresp);
    assign memory.ruser=memory_bus.slave_if[0].ruser;
    assign memory.rlast=memory_bus.slave_if[0].rlast;
    assign memory_bus.slave_if[0].rready=memory.rready;
    always @(posedge control.bus_clk) if(control.bus_rst_n && control.awatop!==0)
        $fatal(1,"COPY_MUST_NOT_ISSUE_ATOMIC");
endmodule
