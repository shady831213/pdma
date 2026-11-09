virtual class pdma_error_irq_test extends pdma_base_test;
    protected bit writing;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(96);
        pdma_channel channel=flow_.channel;
        int delivered_before=channel.delivered;
        job.cross_page=1;
        flow_.open(job);
        inject(writing,0,(writing ? job.dest_address : job.source_address)+pdma_configuration::MEMORY_BEAT_BYTES);
        job.expected_error_prefix=platform.fault_address-(writing ? job.dest_address : job.source_address);
        channel.write_reg(channel.registers.control_block.irq_block.mask_reg,
            (32'b1 << channel.registers.control_block.irq_block.mask_reg.event_non_empty_field.get_lsb_pos()) |
            (32'b1 << channel.registers.control_block.irq_block.mask_reg.trap_field.get_lsb_pos()));
        flow_.submit();
        channel.await_reg(channel.registers.event_block.producer_reg,'1,1);
        @(negedge env.control.core_clk); env.control.irq_ready[channel.channel_id]=1;
        flow_.expect_interrupt(TASK_ERROR,writing ? EVENT_CODE_WRITEERROR : EVENT_CODE_READERROR);
        @(negedge env.control.core_clk); env.control.irq_ready[channel.channel_id]=0;
        if(channel.delivered!=delivered_before+1) `uvm_fatal("IRQ_DELIVERY","Unexpected channel notification count")
        platform.fault_enabled=0; flow_.close(); cases++;
    endtask
endclass

class pdma_read_error_irq_test extends pdma_error_irq_test;
    `uvm_component_utils(pdma_read_error_irq_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); writing=0;
    endfunction
endclass

class pdma_write_error_irq_test extends pdma_error_irq_test;
    `uvm_component_utils(pdma_write_error_irq_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); writing=1;
    endfunction
endclass
