virtual class pdma_fault_test extends pdma_base_test;
    protected bit writing, fence_target;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_task job=settings(pdma_configuration::COPY_BURST_BYTES<64 ? pdma_configuration::COPY_BURST_BYTES : 64);
        // Mutate only bytes retained by this one failed write, never unfetched successors.
        // Round alignment up independently of a non-power-of-two burst length.
        job.payload_alignment=1<<$clog2(job.length);
        flow_.open(job);
        inject(writing,1,fence_target ? job.fence_address : (writing ? job.dest_address : job.source_address));
        flow_.submit(); flow_.expect_result(TASK_FAULT);
        if(writing && !fence_target)
            foreach(job.initial_data[i])
                if(env.memory.read_byte(job.dest_address+i)!==job.initial_data[i])
                    `uvm_fatal("FAULT_SIDE_EFFECT","Failed first payload write modified memory")
        if(writing && !fence_target)
            foreach(job.expected[i]) env.memory.write_byte(job.source_address+i,~job.expected[i]);
        platform.fault_enabled=0;
        flow_.channel.resume_task();
        flow_.channel.await_reg(flow_.channel.registers.control_block.recovery_block.fault_status_reg,
            32'b1 << flow_.channel.registers.control_block.recovery_block.fault_status_reg.faulted_field.get_lsb_pos(),0);
        flow_.expect_result(TASK_SUCCESS); flow_.close(); cases++;
    endtask
endclass

class pdma_w_first_fault_test extends pdma_fault_test;
    `uvm_component_utils(pdma_w_first_fault_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); writing=1; fence_target=1;
    endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        // Deliberately longer than the configured maximum burst.
        platform.write_address_delay=2*pdma_configuration::COPY_BURST_BEATS; platform.write_data_ready_delay=0;
        platform.delay_max=0;
    endfunction
    virtual task run_scenario();
        super.run_scenario();
        if($isunknown(platform.w_first_faults) || platform.w_first_faults<1)
            `uvm_fatal("FAULT_ORDER","Faulting W did not finish before its AW")
    endtask
endclass

class pdma_read_fault_test extends pdma_fault_test;
    `uvm_component_utils(pdma_read_fault_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); writing=0; fence_target=0;
    endfunction
endclass

class pdma_write_fault_test extends pdma_fault_test;
    `uvm_component_utils(pdma_write_fault_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); writing=1; fence_target=0;
    endfunction
endclass

class pdma_fence_fault_test extends pdma_fault_test;
    `uvm_component_utils(pdma_fence_fault_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); writing=1; fence_target=1;
    endfunction
endclass
