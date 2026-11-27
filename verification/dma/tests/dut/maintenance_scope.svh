class pdma_drain_publication_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_drain_publication_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow older=env.device.task_flow(), later=env.device.task_flow();
        pdma_wait_task target=dependency(), unrelated=dependency();
        unrelated.source_context=platform.ring_context;
        older.open(target); later.open(unrelated);
        older.submit(); await_head(older);
        env.control.request_maintenance(0,target.source_context,1);

        // SW-02 permits publication using other contexts during Drain.
        // The newer Wait and its Fence stay on the same physical ring.
        later.submit();
        if(env.control.maint_ready!==0 || target.read_fence(env.memory)!==0)
            `uvm_fatal("DRAIN_FRONTIER","Drain retired the original dependency before it was satisfied")
        target.store(env.memory,target.target_value);
        older.expect_fence();
        await_head(later);
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_Success,0,platform.poll_limit);
        older.close();
        unrelated.store(env.memory,unrelated.target_value);
        later.expect_fence(); later.close(); cases++;
    endtask
endclass

class pdma_contended_wait_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_contended_wait_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flows[`PDMA_CHANNEL_COUNT];
        pdma_wait_task waits[`PDMA_CHANNEL_COUNT];
        foreach(flows[owner]) begin
            flows[owner]=env.device.task_flow(owner);
            waits[owner]=dependency();
            flows[owner].open(waits[owner]); flows[owner].submit(); await_head(flows[owner]);
        end
        env.control.request_maintenance(0,waits[0].source_context,'1);
        // The highest-numbered owner must progress even while every lower
        // owner is still polling and holds a successor Fence in its ring.
        for(int owner=platform.channel_count-1;owner>=0;owner--) begin
            if(env.control.maint_ready!==0)
                `uvm_fatal("WAIT_OWNERS","Drain finished with an unsatisfied dependency")
            for(int pending=0;pending<=owner;pending++)
                if(waits[pending].read_fence(env.memory)!==0)
                    `uvm_fatal("WAIT_OWNERS",$sformatf("Premature Fence on channel %0d",pending))
            waits[owner].store(env.memory,waits[owner].target_value);
            flows[owner].expect_fence();
        end
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_Success,0,platform.poll_limit);
        foreach(flows[owner]) flows[owner].close();
        cases++;
    endtask
endclass

class pdma_maintenance_backpressure_test extends pdma_maintenance_test;
    `uvm_component_utils(pdma_maintenance_backpressure_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_wait_task job=dependency();
        flow_.open(job); flow_.submit(); await_head(flow_);
        env.control.request_maintenance(0,job.source_context,1);
        fork
            env.control.request_maintenance(1,platform.ring_context,1);
            begin
                wait(env.control.maint_valid && env.control.maint_abort);
                // Keep the first operation blocked across several sampling edges.
                repeat(3) begin
                    @(negedge env.control.core_clk);
                    if(env.control.maint_ready!==0 || env.control.maint_valid!==1 ||
                       env.control.maint_status!==pdma_rtl_MaintenanceState::Status_Busy)
                        `uvm_fatal("MAINT_READY","Second request displaced the pending Drain")
                end
                job.store(env.memory,job.target_value);
                env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_Success,0,platform.poll_limit);
            end
        join
        env.control.expect_maintenance(pdma_rtl_MaintenanceState::Status_RingBound,1,platform.poll_limit);
        flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass
