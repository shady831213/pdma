class pdma_irq_enable_level_test extends pdma_base_test;
    `uvm_component_utils(pdma_irq_enable_level_test)
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_task job=settings(16);
        pdma_channel channel=flow_.channel;
        pdma_result wake;
        flow_.open(job);
        channel.write_reg(channel.registers.control_block.irq_block.enable_reg,0);
        job.opcode=COMMANDHEADER_OPCODE_ATOMIC;
        flow_.submit(); flow_.expect_error(EVENT_CODE_UNSUPPORTED);
        channel.write_reg(channel.registers.control_block.irq_block.enable_reg,
            32'b1 << channel.registers.control_block.irq_block.enable_reg.enabled_field.get_lsb_pos());
        flow_.wait_irq_result(wake);
        if(!wake.cause[channel.registers.control_block.irq_block.cause_reg.error_field.get_lsb_pos()] || env.control.irq[channel.channel_id]!==0)
            `uvm_fatal("IRQ_REPLAY","ISR must observe the retained stopping error and disable its IRQ")
        channel.reset(); flow_.close(); cases++;
    endtask
endclass
