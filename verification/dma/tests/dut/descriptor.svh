class pdma_unsupported_command_test extends pdma_base_test;
    `uvm_component_utils(pdma_unsupported_command_test)
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(16);
        job.expect_payload_unchanged=1;
        flow_.open(job);
        job.opcode=COMMANDHEADER_OPCODE_TRAP;
        flow_.submit(); flow_.expect_result(TASK_ERROR,EVENT_CODE_UNSUPPORTED); flow_.close(); cases++;
    endtask
endclass

class pdma_malformed_command_test extends pdma_base_test;
    `uvm_component_utils(pdma_malformed_command_test)
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(16);
        job.expect_payload_unchanged=1;
        flow_.open(job);
        job.opcode='1;
        flow_.submit(); flow_.expect_result(TASK_FATAL,pdma_rtl_ChannelRecoveryLayout::FATAL_CODE_COMMANDREAD);
        flow_.close(); cases++;
    endtask
endclass

class pdma_zero_length_copy_test extends pdma_base_test;
    `uvm_component_utils(pdma_zero_length_copy_test)
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(16);
        job.length=0;
        job.expect_payload_unchanged=1;
        flow_.open(job);
        flow_.submit(); flow_.expect_result(TASK_SUCCESS); flow_.close(); cases++;
    endtask
endclass

class pdma_overflowing_copy_test extends pdma_base_test;
    `uvm_component_utils(pdma_overflowing_copy_test)
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(16);
        job.expect_payload_unchanged=1;
        flow_.open(job);
        job.source_base='1;
        flow_.submit(); flow_.expect_result(TASK_ERROR,EVENT_CODE_INVALIDLENGTH); flow_.close(); cases++;
    endtask
endclass
