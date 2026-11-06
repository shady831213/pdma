virtual class pdma_fatal_test extends pdma_base_test;
    protected bit event_failure;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(16);
        flow_.open(job);
        if(event_failure) job.opcode=COMMANDHEADER_OPCODE_TRAP;
        inject(event_failure,0,event_failure ? flow_.channel.event_address : flow_.channel.next_command_address());
        flow_.submit(); flow_.expect_result(TASK_FATAL,event_failure ? 2 : 1);
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
