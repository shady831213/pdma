virtual class pdma_fatal_test extends pdma_base_test;
    protected bit event_failure;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        expected_cases=platform.channel_count*(platform.channel_count>1 ? 2 : 1);
    endfunction
    virtual task run_scenario();
        foreach(env.device.channels[i]) begin
            pdma_task_flow failed=env.device.task_flow(i);
            pdma_task job=settings(16);
            job.expect_payload_unchanged=1;
            if(event_failure) job.opcode=COMMANDHEADER_OPCODE_ATOMIC;
            failed.open(job);
            inject(event_failure,0,event_failure ? failed.channel.event_address : failed.channel.next_command_address());
            failed.submit();
            if(event_failure) failed.expect_interrupt(TASK_FATAL,pdma_rtl_ChannelRecoveryLayout::FATAL_CODE_EVENTWRITE);
            else failed.expect_fatal(pdma_rtl_ChannelRecoveryLayout::FATAL_CODE_COMMANDREAD);
            platform.fault_enabled=0;
            if(platform.channel_count>1) begin
                pdma_task_flow peer=env.device.task_flow((i+1)%platform.channel_count);
                peer.open(settings()); peer.submit(); peer.expect_fence(); peer.close(); cases++;
            end
            if(env.control.fatal_irq!==0 || env.device.driver.fatal_irqs_serviced!=0)
                `uvm_fatal("FATAL_SCOPE","Channel failure raised a device fatal interrupt")
            failed.channel.reset(); failed.close(); cases++;
        end
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

class pdma_invalid_event_geometry_test extends pdma_base_test;
    `uvm_component_utils(pdma_invalid_event_geometry_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        // Backing exists, but three entries cannot form a valid ring.
        platform.event_capacity=3; expected_cases=platform.channel_count;
    endfunction
    virtual task run_scenario();
        foreach(env.device.channels[i]) begin
            pdma_task_flow flow_=env.device.task_flow(i);
            pdma_task job=settings(16);
            job.opcode=COMMANDHEADER_OPCODE_ATOMIC; job.expect_payload_unchanged=1;
            flow_.open(job);
            for(int b=0;b<platform.event_capacity*EVENT_SIZE;b++)
                env.memory.write_byte(flow_.channel.event_address+b,8'ha5);
            flow_.submit(); flow_.expect_fatal(pdma_rtl_ChannelRecoveryLayout::FATAL_CODE_EVENTCONFIGURATION);
            for(int b=0;b<platform.event_capacity*EVENT_SIZE;b++)
                if(env.memory.read_byte(flow_.channel.event_address+b)!==8'ha5)
                    `uvm_fatal("EVENT_GEOMETRY","DMA wrote an event to an invalid ring")
            flow_.channel.reset(); flow_.close(); cases++;
        end
    endtask
endclass
