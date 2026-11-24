class pdma_sg_endpoints_test extends pdma_base_test;
    `uvm_component_utils(pdma_sg_endpoints_test)
    function new(string name,uvm_component parent); super.new(name,parent); expected_cases=4; endfunction
    virtual task run_scenario();
        for(int mode=0;mode<4;mode++) begin
            pdma_task_flow flow_=env.device.task_flow();
            pdma_sg_task job=new;
            job.source_sg=mode[0]; job.dest_sg=mode[1];
            flow_.open(job); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
        end
    endtask
endclass

class pdma_sg_validation_test extends pdma_base_test;
    `uvm_component_utils(pdma_sg_validation_test)
    function new(string name,uvm_component parent); super.new(name,parent); expected_cases=2; endfunction
    virtual task run_scenario();
        for(int fail_read=0;fail_read<2;fail_read++) begin
            pdma_task_flow flow_=env.device.task_flow();
            pdma_sg_task job=new;
            job.short_destination=!fail_read;
            job.expect_payload_unchanged=1;
            flow_.open(job);
            // Make even an early source payload read observable before the late validation error.
            env.memory.release_regions(job.source_buffer.allocation.get_s_addr());
            if(fail_read) inject(0,1,job.tables[1].address+job.SECOND*SEGMENT_SIZE);
            flow_.submit();
            flow_.expect_error(fail_read ? EVENT_CODE_LISTREADERROR : EVENT_CODE_LENGTHMISMATCH);
            platform.fault_enabled=0; flow_.channel.reset(); flow_.close(); cases++;
        end
    endtask
endclass

class pdma_sg_recovery_test extends pdma_base_test;
    `uvm_component_utils(pdma_sg_recovery_test)
    function new(string name,uvm_component parent); super.new(name,parent); expected_cases=2; endfunction
    virtual task run_scenario();
        for(int reset_instead=0;reset_instead<2;reset_instead++) begin
            pdma_task_flow flow_=env.device.task_flow();
            pdma_sg_task job=new;
            int unsigned boundary=job.first_length(1);
            flow_.open(job);
            inject(1,1,job.destination_byte(boundary));
            flow_.submit(); flow_.expect_fault();
            for(int i=0;i<boundary;i++)
                if(env.memory.read_byte(job.destination_byte(i))!==job.expected[i])
                    `uvm_fatal("SG_PREFIX","Earlier span did not finish before the fault")
            if(platform.channel_count>1) begin
                pdma_task_flow peer=env.device.task_flow(1);
                peer.open(settings(64)); peer.submit(); peer.expect_fence(); peer.close();
            end
            platform.fault_enabled=0;
            if(reset_instead) begin
                flow_.channel.request_reset(); flow_.cancel();
                flow_.open(settings(64)); flow_.submit();
            end else begin
                for(int i=boundary;i<job.length;i++) env.memory.write_byte(job.address_at(0,i),~job.expected[i]);
                flow_.channel.resume_task();
            end
            flow_.expect_fence(); flow_.close(); cases++;
        end
    endtask
endclass
