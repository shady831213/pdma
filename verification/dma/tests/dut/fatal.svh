virtual class pdma_fatal_test extends pdma_base_test;
    protected bit event_failure;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(16);
        flow_.open(job);
        if(event_failure) job.opcode=COMMANDHEADER_OPCODE_ATOMIC;
        inject(event_failure,0,event_failure ? flow_.channel.event_address : flow_.channel.next_command_address());
        flow_.submit(); flow_.expect_result(TASK_FATAL,event_failure ?
            pdma_rtl_ChannelRecoveryLayout::FATAL_CODE_EVENTWRITE : pdma_rtl_ChannelRecoveryLayout::FATAL_CODE_COMMANDREAD);
        platform.fault_enabled=0; flow_.close(); cases++;
    endtask
endclass

class pdma_command_read_fatal_test extends pdma_fatal_test;
    `uvm_component_utils(pdma_command_read_fatal_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); event_failure=0;
    endfunction
endclass

class pdma_event_write_fatal_test extends pdma_fatal_test;
    `uvm_component_utils(pdma_event_write_fatal_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); event_failure=1;
    endfunction
endclass

class pdma_device_fatal_irq_test extends pdma_base_test;
    `uvm_component_utils(pdma_device_fatal_irq_test)
    function new(string name,uvm_component parent); super.new(name,parent); expected_cases=1; endfunction
    virtual task run_scenario();
        pdma_task_flow failed=env.task_flow(platform.channel_count-1);
        pdma_task job=settings(16);
        job.expect_payload_unchanged=1;
        failed.open(job); inject(0,2'b10,failed.channel.next_command_address()); failed.submit();
        env.device.await_reg(env.device.registers.fatal_reg,'1,pdma_rtl_DmaControlLayout::FATAL_CODE_INTERNAL);
        for(int n=0;n<platform.irq_timeout_cycles && !env.device_handler.delivered;n++) @(negedge env.control.core_clk);
        if(env.device_handler.delivered!=1 || env.control.fatal_irq!==0)
            `uvm_fatal("DEVICE_IRQ_SERVICE","Fatal handler must observe cause and disable its own IRQ")
        platform.fault_enabled=0; env.device.request_reset(); failed.cancel(1); cases++;
    endtask
endclass
