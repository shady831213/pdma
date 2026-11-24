virtual class pdma_fatal_test extends pdma_base_test;
    protected bit event_failure;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_task job=settings(16);
        flow_.open(job);
        if(event_failure) job.opcode=COMMANDHEADER_OPCODE_ATOMIC;
        inject(event_failure,0,event_failure ? flow_.channel.event_address : flow_.channel.next_command_address());
        flow_.submit();
        if(event_failure) flow_.expect_interrupt(TASK_FATAL,pdma_rtl_ChannelRecoveryLayout::FATAL_CODE_EVENTWRITE);
        else flow_.expect_fatal(pdma_rtl_ChannelRecoveryLayout::FATAL_CODE_COMMANDREAD);
        platform.fault_enabled=0; flow_.channel.reset(); flow_.close(); cases++;
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
        super.new(name,parent); event_failure=1; interrupt_driven=1;
    endfunction
endclass

class pdma_device_fatal_irq_test extends pdma_base_test;
    `uvm_component_utils(pdma_device_fatal_irq_test)
    function new(string name,uvm_component parent); super.new(name,parent); expected_cases=1; endfunction
    virtual task run_scenario();
        pdma_task_flow failed=env.device.task_flow(platform.channel_count-1);
        pdma_task job=settings(16);
        jvs_irq_result_trans irq;
        pdma_result observation;
        job.expect_payload_unchanged=1;
        failed.open(job); inject(0,2'b10,failed.channel.next_command_address()); failed.submit();
        env.device.fatal_irq.handler.get_irq_result(irq,platform.irq_timeout_ns);
        if(irq==null) `uvm_fatal("TASK_TIMEOUT","Device fatal IRQ did not arrive")
        if(!$cast(observation,irq.attr) || observation==null)
            `uvm_fatal("DEVICE_IRQ_SERVICE","Missing device fatal observation")
        if(observation.outcome!=TASK_FATAL || observation.code!==pdma_rtl_DmaControlLayout::FATAL_CODE_INTERNAL)
            `uvm_fatal("DEVICE_IRQ_CAUSE","Unexpected device fatal reason")
        if(env.device.driver.fatal_irqs_serviced!=1 || env.control.fatal_irq!==0)
            `uvm_fatal("DEVICE_IRQ_SERVICE","Fatal handler must observe cause and disable its own IRQ")
        platform.fault_enabled=0; env.device.driver.request_reset(); failed.cancel(1); cases++;
    endtask
endclass
