virtual class pdma_maintenance_test extends pdma_base_test;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    protected function pdma_wait_task dependency();
        pdma_wait_task job=new;
        job.source_context=~platform.ring_context;
        job.fence_context=platform.ring_context;
        job.target_value=1;
        return job;
    endfunction
    protected task await_head(pdma_task_flow flow_);
        pdma_channel ch=flow_.channel;
        ch.await_reg(ch.registers.command_block.consumer_reg,'1,ch.producer-1);
        if(flow_.job.read_fence(env.memory)!==0)
            `uvm_fatal("WAIT_ORDER","Successor Fence passed an unsatisfied dependency")
    endtask
endclass

class pdma_wait_drain_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_wait_drain_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=platform.channel_count;
    endfunction
    virtual task run_scenario();
        foreach(env.device.channels[owner]) drain_owner(owner);
    endtask
    protected task drain_owner(int owner);
        pdma_task_flow flow_=env.device.task_flow(owner);
        pdma_wait_task job=dependency();
        uvm_status_e status;
        bit [63:0] initial_value=0, satisfied_value=1;
        job.width=owner%2 ? 8 : 4;
        case(owner%3)
            0: job.target_value=job.width==4 ? 64'hffff_ffff_0000_0001 : 1;
            1: begin
                job.comparison=WAITCOMMAND_COMPARISON_SIGNEDLESS;
                job.target_value=-1; satisfied_value=-2;
            end
            2: begin
                job.comparison=WAITCOMMAND_COMPARISON_REACHED;
                job.target_value=1; initial_value=-2; satisfied_value=2;
            end
        endcase
        flow_.open(job); job.store(env.memory,initial_value);
        flow_.submit(); await_head(flow_);
        env.control.request_maintenance(0,job.source_context,1<<owner);
        flow_.channel.registers.control_block.config_block.enable_reg.write(status,0,UVM_FRONTDOOR);
        if(status!==UVM_NOT_OK)
            `uvm_fatal("MAINT_CONFIG","Active maintenance permitted a change of enable")
        if(platform.channel_count>1) begin
            pdma_task_flow peer=env.device.task_flow((owner+1)%platform.channel_count);
            pdma_task other=settings();
            peer.open(other); peer.submit(); peer.expect_fence(); peer.close();
        end
        if(env.control.maint_status!==pdma_rtl_MaintenanceState::Status_Busy || job.read_fence(env.memory)!==0)
            `uvm_fatal("WAIT_DRAIN","Drain or successor Fence completed before the dependency")
        job.store(env.memory,satisfied_value);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_Success,0,platform.poll_limit);
        flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass

class pdma_wait_abort_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_wait_abort_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_wait_task job=dependency();
        Command_t commands[$];
        WaitCommand_t second;
        job.fence_context=job.source_context;
        flow_.open(job);
        commands.push_back(job.command_record());
        second=WaitCommand_t'(job.command_record());
        WaitCommand_set_id(second,flow_.channel.next_id++);
        commands.push_back(Command_t'(second));
        commands.push_back(job.fence_record());
        flow_.submit_records(commands);
        flow_.channel.await_reg(flow_.channel.registers.command_block.consumer_reg,'1,flow_.channel.producer-2);
        env.control.request_maintenance(1,platform.ring_context,'1);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_RingBound,'1,platform.poll_limit);
        env.control.request_maintenance(1,job.source_context,1);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_Success,0,platform.poll_limit);
        flow_.confirm_discard();
        if(env.memory.read_long(job.source_address)!==0)
            `uvm_fatal("WAIT_ABORT","Dependency word changed during cancellation")
        flow_.close();
        flow_.open(settings()); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass

class pdma_wait_reset_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_wait_reset_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_wait_task job=dependency();
        flow_.open(job); flow_.submit(); await_head(flow_);
        env.control.request_maintenance(0,job.source_context,1);
        flow_.channel.request_reset();
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_ChannelStopped,1,platform.poll_limit);
        flow_.cancel();
        flow_.open(settings()); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass

class pdma_wait_fault_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_wait_fault_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        int last=platform.channel_count-1;
        bit [`PDMA_CHANNEL_COUNT-1:0] scope=(1<<last)|1, failed='0;
        bit [31:0] target=~platform.ring_context;
        pdma_task_flow parked[$];
        for(int i=0;i<(last==0 ? 1 : 2);i++) begin
            int owner=i==0 ? last : 0;
            pdma_task_flow flow_=env.device.task_flow(owner);
            pdma_wait_task job=dependency();
            job.fence_context=job.source_context;
            flow_.open(job); inject(0,1,job.source_address); flow_.submit(); flow_.expect_fault();
            parked.push_back(flow_);
            failed[owner]=1;
            env.control.request_maintenance(0,target,scope);
            env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_ChannelStopped,failed,platform.poll_limit);
        end
        env.control.request_maintenance(0,platform.ring_context,scope);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_RingBound,scope,platform.poll_limit);
        // An unrelated parked task still blocks reaching the captured Fence.
        env.control.request_maintenance(1,target^1,scope);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_ChannelStopped,scope,platform.poll_limit);
        env.control.request_maintenance(1,target,scope);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_Success,0,platform.poll_limit);
        env.control.request_maintenance(0,platform.ring_context,scope);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_RingBound,scope,platform.poll_limit);
        platform.fault_enabled=0;
        foreach(parked[i]) begin
            parked[i].confirm_discard(); parked[i].close();
            parked[i].open(settings()); parked[i].submit(); parked[i].expect_fence(); parked[i].close();
        end
        cases++;
    endtask
endclass

class pdma_wait_read_error_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_wait_read_error_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_wait_task job=dependency();
        flow_.open(job); inject(0,0,job.source_address); flow_.submit();
        flow_.expect_error(EVENT_CODE_READERROR);
        platform.fault_enabled=0; flow_.channel.reset(); flow_.close(); cases++;
    endtask
endclass

class pdma_wait_resume_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_wait_resume_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_wait_task job=dependency();
        job.target_value=64'hfedc_ba98_0123_4567;
        flow_.open(job); inject(0,1,job.source_address); flow_.submit(); flow_.expect_fault();
        job.store(env.memory,job.target_value);
        platform.fault_enabled=0; flow_.channel.resume_task();
        flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass

class pdma_context_linear_selector_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_context_linear_selector_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow blocked=env.device.task_flow(), copy_flow=env.device.task_flow();
        pdma_wait_task job=dependency();
        pdma_sg_task linear=new;
        job.fence_context=job.source_context;
        linear.source_sg=0; linear.dest_sg=0; linear.list_context=job.source_context;
        linear.source_context=platform.ring_context; linear.dest_context=platform.ring_context;
        linear.fence_context=platform.ring_context;
        blocked.open(job); blocked.submit(); await_head(blocked);
        copy_flow.open(linear); copy_flow.submit();
        env.control.request_maintenance(1,job.source_context,1);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_Success,0,platform.poll_limit);
        blocked.confirm_discard(); blocked.close();
        copy_flow.expect_fence(); copy_flow.close(); cases++;
    endtask
endclass

class pdma_context_copy_abort_test extends pdma_base_test;
    `uvm_component_utils(pdma_context_copy_abort_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_sg_task job=new;
        // Fault the first source read. No destination byte has been written.
        // The following Fence uses the same selector as the SG lists.
        job.list_context=~platform.ring_context; job.fence_context=job.list_context;
        job.expect_payload_unchanged=1;
        flow_.open(job); inject(0,1,job.address_at(0,0)); flow_.submit(); flow_.expect_fault();
        env.control.request_maintenance(1,job.list_context,1);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_Success,0,platform.poll_limit);
        flow_.confirm_discard(); flow_.close();
        platform.fault_enabled=0;
        // Reuse both the channel's SG walker and its burst rows.
        job=new; flow_.open(job); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass

class pdma_context_running_abort_test extends pdma_base_test;
    `uvm_component_utils(pdma_context_running_abort_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_pattern_task job=new;
        Command_t command[$],fence[$];
        uvm_reg_data_t consumer;
        realtime fence_published;
        job.length=(`PDMA_COPY_SLOTS+2)*pdma_configuration::COPY_BURST_BYTES;
        job.dest_context=~platform.ring_context; job.fence_context=platform.ring_context;
        flow_.open(job); platform.hold_write=1;
        command.push_back(job.command_record()); flow_.submit_records(command);
        wait(platform.held.size()!=0);
        env.control.request_maintenance(1,job.dest_context,1);
        flow_.channel.read_reg(flow_.channel.registers.command_block.consumer_reg,consumer);
        if(consumer!==flow_.channel.producer || env.control.maint_ready!==0)
            `uvm_fatal("ABORT_RUNNING","An empty frontier retired a Fill with an outstanding write")
        fence.push_back(job.fence_record()); flow_.channel.publish(fence,fence_published);
        platform.hold_write=0;
        foreach(platform.held[i]) platform.held[i].suspend_response=0;
        platform.held.delete();
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_Success,0,platform.poll_limit);
        flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass
