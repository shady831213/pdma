module pdma_vip_dut(svt_axi_if host_bus, svt_axi_if memory_bus, pdma_control_if control);
    `include "dut_binding.svh"
    import common_cell_ready_valid_properties::held_until_accepted;
    register_request: assert property (held_until_accepted(control.reg_clk, control.reg_rst_n,
        integration.dma.register_port.request_valid, integration.dma.register_port.request_ready,
        integration.dma.register_port.request)) else $fatal(1, "register_request changed while stalled");
    register_response: assert property (held_until_accepted(control.reg_clk, control.reg_rst_n,
        integration.dma.register_port.response_valid, integration.dma.register_port.response_ready,
        integration.dma.register_port.response)) else $fatal(1, "register_response changed while stalled");
    burst_request: assert property (held_until_accepted(control.bus_clk, control.bus_rst_n,
        integration.dma.transactions.valid, integration.dma.transactions.ready,
        integration.dma.transactions.request)) else $fatal(1, "burst_request changed while stalled");
    burst_response: assert property (held_until_accepted(control.bus_clk, control.bus_rst_n,
        integration.dma.transactions.returned, integration.dma.transactions.accepted,
        integration.dma.transactions.receipt)) else $fatal(1, "burst_response changed while stalled");
    pdma_axi_activity #(.ID_BITS(`PDMA_ID_BITS)) activity(
        .clk(control.bus_clk),.rst_n(control.bus_rst_n),
        .ar(memory.arvalid && memory.arready),.arid(memory.arid),
        .aw(memory.awvalid && memory.awready),.awid(memory.awid),
        .r(memory.rvalid && memory.rready),.rid(memory.rid),.rlast(memory.rlast),
        .b(memory.bvalid && memory.bready),.bid(memory.bid)
    );
    pdma_pipeline_timing #(.CHANNELS(`PDMA_CHANNEL_COUNT),.SLOTS(`PDMA_COPY_SLOTS)) timing_probe(
        .core_clk(control.core_clk),.core_rst_n(control.core_rst_n),
        .bus_clk(control.bus_clk),.bus_rst_n(control.bus_rst_n),
        .grant(!integration.dma.commands.execution.locked && |integration.dma.commands.execution.choice),.chosen(integration.dma.commands.execution.chosen),
        .copy_start(integration.dma.commands.execution.copy_start),
        .issued(integration.dma.requests.valid && integration.dma.requests.ready),
        .issue_channel(integration.dma.requests.request.tag.channel),.issue_slot(integration.dma.requests.request.tag.slot),
        .issue_write(integration.dma.requests.request.write),
        .retired(integration.dma.requests.returned && !integration.dma.requests.receipt.orphan && integration.dma.requests.accepted),
        .retire_channel(integration.dma.requests.receipt.tag.channel),.retire_slot(integration.dma.requests.receipt.tag.slot),
        .fetch_admitted(integration.dma.fetch_valid && integration.dma.fetch_ready), .fetch_channel(integration.dma.fetch.channel),
        .admitted(integration.dma.transactions.valid && integration.dma.transactions.ready),
        .admit_channel(integration.dma.transactions.request.tag.channel),.admit_slot(integration.dma.transactions.request.tag.slot),
        .returned(integration.dma.transactions.returned && !integration.dma.transactions.receipt.orphan && integration.dma.transactions.accepted),
        .return_channel(integration.dma.transactions.receipt.tag.channel),.return_slot(integration.dma.transactions.receipt.tag.slot)
    );
    for(genvar c=0;c<`PDMA_CHANNEL_COUNT;c++) begin: channels
        // Follow this channel's stored context even while a peer owns the engine.
        `define PDMA_EXEC integration.dma.commands.execution
        pdma_vip_pkg::pdma_execution_coverage phases;
        pdma_rtl_CommandState::Phase prior_command;
        int unsigned prior_segment;
        bit sample_command, sample_segment;
        initial phases=new($sformatf("channel_%0d",c),
            `PDMA_EXEC.segments.Phase_SelectSide,`PDMA_EXEC.segments.Phase_Locate,
            `PDMA_EXEC.segments.Phase_Read,`PDMA_EXEC.segments.Phase_Receive,
            `PDMA_EXEC.segments.Phase_Decode,`PDMA_EXEC.segments.Phase_Apply,
            `PDMA_EXEC.segments.Phase_Offer,`PDMA_EXEC.segments.Phase_Payload);
        always @(posedge control.core_clk or negedge control.core_rst_n) begin
            if(!control.core_rst_n) begin
                sample_command=0; sample_segment=0;
            end else begin
                if(sample_command && prior_command!=`PDMA_EXEC.contexts[c].phase)
                    phases.commands.sample(prior_command,`PDMA_EXEC.contexts[c].phase);
                if(sample_segment && prior_segment!=`PDMA_EXEC.segments.states[c].phase)
                    phases.segments.sample(prior_segment,`PDMA_EXEC.segments.states[c].phase);
                prior_command=`PDMA_EXEC.contexts[c].phase;
                prior_segment=`PDMA_EXEC.segments.states[c].phase;
                sample_command=!`PDMA_EXEC.copy_cancel[c] && !integration.dma.channels.clear[c];
                sample_segment=sample_command &&
                    !(`PDMA_EXEC.segments.start && `PDMA_EXEC.selected==c);
            end
        end
        `undef PDMA_EXEC
        pdma_lifecycle_assertions #(.SLOTS(`PDMA_COPY_SLOTS)) lifecycle(
            .clk(control.core_clk),.rst_n(control.core_rst_n),
            .request(integration.dma.requests.valid && integration.dma.requests.request.tag.channel==c),.ready(integration.dma.requests.ready),
            .returned(integration.dma.requests.returned && !integration.dma.requests.receipt.orphan && integration.dma.requests.receipt.tag.channel==c),.accepted(integration.dma.requests.accepted),
            .request_slot(integration.dma.requests.request.tag.slot),.receipt_slot(integration.dma.requests.receipt.tag.slot),
            .closing(integration.dma.commands.execution.shutdown[c]),.clear(integration.dma.channels.clear[c]),
            .enabled(integration.dma.channels.configuration[c].enable),.has_commands(integration.dma.channels.command[c].producer!=integration.dma.channels.command[c].consumer),
            .irq(control.irq[c]),
            .irq_source(integration.dma.channels.configuration[c].irq_enable && (
                integration.dma.channels.status[c].error || integration.dma.channels.status[c].fatal!=0 ||
                (!integration.dma.channels.configuration[c].irq_mask[pdma_rtl_ChannelIrqMaskLayout::VALUE_TRAP_LSB] &&
                    integration.dma.control.csr.owners.channels[c].owner.trap_cause) ||
                (!integration.dma.channels.configuration[c].irq_mask[pdma_rtl_ChannelIrqMaskLayout::VALUE_EVENT_NON_EMPTY_LSB] &&
                    integration.dma.channels.events[c].producer!=integration.dma.channels.events[c].consumer))),
            .parked(integration.dma.channels.status[c].faulted),.resume_request(integration.dma.channels.actions[c].resume_request),
            .discard(integration.dma.commands.execution.discard_saved[c]),
            .scoped_abort(control.maint_valid && control.maint_ready && control.maint_abort && control.maint_channels[c]),
            .maintenance_busy(!control.maint_ready),
            .parked_state({(integration.dma.commands.execution.locked && integration.dma.commands.execution.selected==c ?
                integration.dma.commands.execution.descriptor : integration.dma.commands.execution.parked_descriptor[c]),
                integration.dma.commands.execution.copy.next_source[c],integration.dma.commands.execution.copy.next_destination[c],
                integration.dma.commands.execution.copy.remaining[c],integration.dma.commands.execution.copy.committed[c]})
        );
    end
    assert property(@(posedge control.core_clk) disable iff(!control.core_rst_n)
        integration.dma.requests.valid |-> integration.dma.requests.request.tag.channel<`PDMA_CHANNEL_COUNT)
        else $fatal(1,"DMA_INVALID_REQUEST_OWNER");
    assert property(@(posedge control.core_clk) disable iff(!control.core_rst_n)
        integration.dma.requests.returned |-> integration.dma.requests.receipt.tag.channel<`PDMA_CHANNEL_COUNT)
        else $fatal(1,"DMA_INVALID_RECEIPT_OWNER");
    assert property(@(posedge control.core_clk) disable iff(!control.core_rst_n)
        control.fatal_irq == (integration.dma.control.global_irq_enable && control.fatal))
        else $fatal(1,"DMA_FATAL_IRQ_LEVEL");
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
