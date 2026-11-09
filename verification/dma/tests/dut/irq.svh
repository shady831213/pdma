class pdma_irq_enable_no_replay_test extends pdma_base_test;
    `uvm_component_utils(pdma_irq_enable_no_replay_test)
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(16);
        pdma_channel channel=flow_.channel;
        int delivered_before=channel.delivered;
        flow_.open(job);
        channel.write_reg(channel.registers.control_block.irq_block.enable_reg,0);
        job.opcode=COMMANDHEADER_OPCODE_TRAP;
        flow_.submit(); flow_.expect_result(TASK_ERROR,EVENT_CODE_UNSUPPORTED);
        channel.write_reg(channel.registers.control_block.irq_block.enable_reg,
            32'b1 << channel.registers.control_block.irq_block.enable_reg.enabled_field.get_lsb_pos());
        repeat(100) @(posedge env.control.core_clk);
        if(env.control.irq[channel.channel_id] || channel.delivered!=delivered_before)
            `uvm_fatal("IRQ_REPLAY","Enable replayed an old channel notification")
        flow_.close(); cases++;
    endtask
endclass
