class pdma_aligned_copy_test extends pdma_base_test;
    `uvm_component_utils(pdma_aligned_copy_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); expected_cases=4;
    endfunction
    virtual task run_scenario();
        for(int i=0;i<4;i++) begin
            pdma_task_flow flow_=env.device.task_flow();
            pdma_task job=settings((i+1)*64);
            job.payload_alignment=pdma_configuration::MEMORY_BEAT_BYTES;
            flow_.open(job); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
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
            pdma_task_flow flow_=env.device.task_flow();
            pdma_task job=settings(0,1);
            flow_.open(job); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
        end
    endtask
endclass

class pdma_last_lane_copy_test extends pdma_base_test;
    `uvm_component_utils(pdma_last_lane_copy_test)
    localparam int unsigned BEAT=pdma_configuration::MEMORY_BEAT_BYTES;
    function new(string name,uvm_component parent); super.new(name,parent); expected_cases=2; endfunction
    virtual task run_scenario();
        for(int last_source=0;last_source<=1;last_source++) begin
            pdma_task_flow flow_=env.device.task_flow();
            pdma_task job=new;
            job.max_length=2*BEAT+1;
            if(!job.randomize() with {
                length==2*BEAT+1; cross_page==0;
                source_lane==(local::last_source ? BEAT-1 : 0);
                dest_lane==(local::last_source ? 0 : BEAT-1);
            }) `uvm_fatal("LAST_LANE_CONFIG","Cannot generate a Copy at the final byte lane")
            job.payload_alignment=BEAT;
            flow_.open(job);
            if(job.source_address%BEAT!=job.source_lane || job.dest_address%BEAT!=job.dest_lane)
                `uvm_fatal("LAST_LANE_ADDRESS","Allocated payload does not exercise the selected lanes")
            flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
        end
    endtask
endclass
