class pdma_event_boundary_test extends pdma_base_test;
    `uvm_component_utils(pdma_event_boundary_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        longint unsigned boundary;
        super.build_phase(phase);
        boundary=(platform.heap_base/pdma_configuration::AXI_BOUNDARY_BYTES+1)*pdma_configuration::AXI_BOUNDARY_BYTES;
        platform.event_ring_address={1'b0,boundary-EVENT_ALIGNMENT};
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(16);
        pdma_channel channel=flow_.channel;
        longint unsigned address=channel.event_address;
        int unsigned consumer_before=channel.event_consumer;
        if(address%pdma_configuration::AXI_BOUNDARY_BYTES+EVENT_SIZE<=pdma_configuration::AXI_BOUNDARY_BYTES)
            `uvm_fatal("EVENT_BOUNDARY","Event entry must straddle an AXI page boundary")
        job.opcode=COMMANDHEADER_OPCODE_TRAP;
        job.expect_payload_unchanged=1;
        for(int i=0;i<platform.event_capacity*EVENT_SIZE;i++) env.memory.write_byte(address+i,8'ha5);
        flow_.open(job); flow_.submit(); flow_.expect_result(TASK_ERROR,EVENT_CODE_UNSUPPORTED);
        if(channel.event_consumer!=consumer_before+1)
            `uvm_fatal("EVENT_BOUNDARY","Error event was not consumed exactly once")
        for(int i=EVENT_SIZE;i<platform.event_capacity*EVENT_SIZE;i++)
            if(env.memory.read_byte(address+i)!==8'ha5)
                `uvm_fatal("EVENT_BOUNDARY","Error publication overwrote an adjacent event slot")
        flow_.close(); cases++;
    endtask
endclass
