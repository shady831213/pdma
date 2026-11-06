class pdma_task_flow extends uvm_object;
    `uvm_object_utils(pdma_task_flow)
    pdma_task job;
    pdma_channel channel;
    pdma_task_checker checker_;
    pdma_memory memory;
    jvs_memory_allocator allocator;
    uvm_analysis_port#(pdma_observation) observations;
    bit published;
    pdma_outcome terminal_outcome;
    function new(string name="task_flow"); super.new(name); endfunction
    task open(pdma_task settings);
        job=settings; job.channel_id=channel.channel_id;
        job.id=channel.next_id; channel.next_id+=2;
        if(!channel.configured) channel.configure();
        job.allocate(allocator,memory);
    endtask
    virtual task submit();
        channel.submit(job); published=1;
    endtask
    function void publish_observation(pdma_outcome outcome);
        pdma_observation item=new;
        item.channel_id=job.channel_id; item.task_id=job.id; item.outcome=outcome;
        item.length=job.length; item.source_lane=job.source_lane; item.dest_lane=job.dest_lane;
        item.cross_page=job.cross_page; item.fence_wide=job.fence_wide;
        item.latency=$realtime-job.submitted_at;
        observations.write(item);
    endfunction
    task expect_result(pdma_outcome outcome,int code=0);
        pdma_result result;
        channel.wait_result(job,result);
        checker_.check(job,result,outcome,memory,code);
        terminal_outcome=result.outcome;
        publish_observation(result.outcome);
    endtask
    task expect_interrupt(pdma_outcome outcome,int code=0);
        pdma_result result;
        channel.wait_interrupt(result);
        checker_.interrupt_result(result);
        checker_.check(job,result,outcome,memory,code);
        terminal_outcome=result.outcome;
        publish_observation(result.outcome);
    endtask
    task close();
        if(published && !job.terminal_checked)
            `uvm_fatal("TASK_LIFETIME","Flow attempted cleanup before checking its terminal result")
        if(published && terminal_outcome==TASK_SUCCESS) channel.await_consumed();
        else if(published) begin channel.request_reset(); channel.finish_reset(); end
        job.deallocate(allocator,memory); published=0;
    endtask
    task cancel(bit global_scope=0);
        pdma_result result=new;
        channel.finish_reset(global_scope);
        result.channel_id=channel.channel_id; result.task_id=job.id; result.outcome=TASK_RESET;
        checker_.check(job,result,TASK_RESET,memory);
        publish_observation(TASK_RESET);
        job.deallocate(allocator,memory); published=0;
    endtask
endclass
