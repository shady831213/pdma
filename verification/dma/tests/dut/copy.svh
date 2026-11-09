class pdma_aligned_copy_test extends pdma_base_test;
    `uvm_component_utils(pdma_aligned_copy_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); expected_cases=4;
    endfunction
    virtual task run_scenario();
        for(int i=0;i<4;i++) begin
            pdma_task_flow flow_=env.task_flow();
            pdma_task job=settings((i+1)*64);
            job.payload_alignment=pdma_configuration::MEMORY_BEAT_BYTES;
            flow_.open(job); flow_.submit(); flow_.expect_result(TASK_SUCCESS); flow_.close(); cases++;
        end
    endtask
endclass

class pdma_random_copy_test extends pdma_base_test;
    `uvm_component_utils(pdma_random_copy_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); expected_cases=20;
    endfunction
    virtual task run_scenario();
        for(int i=0;i<20;i++) begin
            pdma_task_flow flow_=env.task_flow();
            pdma_task job=settings(0,1);
            flow_.open(job); flow_.submit(); flow_.expect_result(TASK_SUCCESS); flow_.close(); cases++;
        end
    endtask
endclass
