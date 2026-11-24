virtual class pdma_fault_test extends pdma_base_test;
    protected bit writing, fence_target;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_task job=single_burst_copy();
        flow_.open(job);
        inject(writing,1,fence_target ? job.fence_address : (writing ? job.dest_address : job.source_address));
        flow_.submit(); flow_.expect_fault();
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
        flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass

class pdma_w_first_fault_test extends pdma_fault_test;
    `uvm_component_utils(pdma_w_first_fault_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); writing=1; fence_target=1;
    endfunction
    virtual function void build_phase(uvm_phase phase);
        // Delay AW past the last W of the faulting Fence, including Fence64
        // on a narrow bus. Payload burst capacity is independent of Fence size.
        int unsigned beat_bytes=pdma_configuration::MEMORY_BEAT_BYTES;
        int unsigned fence_beats=(8+beat_bytes-1)/beat_bytes;
        super.build_phase(phase);
        platform.write_address_delay=fence_beats+2; platform.write_data_ready_delay=0;
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

// The failing Fence writes a separate allocation. Its successor's marker
// must remain untouched, and the diagnostic must identify the first Fence.
class pdma_error_fence extends pdma_task;
    virtual function Command_t command_record();
        FenceCommand_t cmd=FenceCommand_t'(fence_record());
        WordWrite_t word_value=FenceCommand_get_word(cmd);
        WordTarget_t target=WordWrite_get_target(word_value);
        WordTarget_set_address(target,dest_address); WordTarget_set_context(target,dest_context);
        WordWrite_set_target(word_value,target); FenceCommand_set_word(cmd,word_value);
        FenceCommand_set_id(cmd,id);
        return Command_t'(cmd);
    endfunction
endclass

class pdma_fence_write_error_test extends pdma_base_test;
    `uvm_component_utils(pdma_fence_write_error_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=platform.channel_count;
    endfunction
    virtual task run_scenario();
        foreach(env.device.channels[i]) begin
            pdma_task_flow flow_=env.device.task_flow(i);
            pdma_error_fence job=new;
            job.length=job.fence_bytes(); job.payload_alignment=job.length;
            job.expect_payload_unchanged=1;
            flow_.open(job); inject(1,0,job.dest_address);
            flow_.submit(); flow_.expect_error(EVENT_CODE_WRITEERROR);
            platform.fault_enabled=0; flow_.channel.reset(); flow_.close(); cases++;
        end
    endtask
endclass
