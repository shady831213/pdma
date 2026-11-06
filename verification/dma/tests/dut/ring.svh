class pdma_ring_wrap_test extends pdma_base_test;
    `uvm_component_utils(pdma_ring_wrap_test)
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual task run_scenario();
        longint unsigned ring_address=env.channels[0].command_address;
        repeat(3) begin
            pdma_task_flow flow_=env.task_flow();
            pdma_task job=settings(32);
            flow_.open(job); flow_.submit(); flow_.expect_result(TASK_SUCCESS); flow_.close();
            if(env.channels[0].command_address!=ring_address)
                `uvm_fatal("RING_LIFETIME","Task close replaced channel ring")
        end
        if(env.channels[0].producer!=6) `uvm_fatal("RING_WRAP","Distinct tasks did not retain producer cursor")
        cases++;
    endtask
endclass
