class pdma_task_checker extends uvm_object;
    `uvm_object_utils(pdma_task_checker)
    int unsigned channel_id, checked_tasks;
    longint unsigned checked_bytes;
    covergroup irq_service with function sample(pdma_outcome outcome);
        option.per_instance=1;
        observed: coverpoint outcome {
            bins notification={IRQ_NOTIFICATION};
            bins diagnostic={TASK_ERROR};
            bins fatal={TASK_FATAL};
            illegal_bins non_irq=default;
        }
    endgroup
    function new(string name="task_checker"); super.new(name); irq_service=new; endfunction
    local function void check_copied_bytes(pdma_task job,pdma_memory memory,int unsigned count);
        for(int i=0;i<count;i++)
            if(memory.read_byte(job.destination_byte(i))!==job.expected[i])
                `uvm_fatal("TASK_DATA",$sformatf("channel=%0d task=%0d destination byte=%0d",channel_id,job.id,i))
        checked_bytes+=count;
    endfunction
    virtual function void check(pdma_task job,pdma_result result,pdma_outcome expected_outcome,
        pdma_memory memory,int expected_code=0);
        if(result.channel_id!==channel_id || job.channel_id!==channel_id ||
            (expected_outcome inside {TASK_SUCCESS,TASK_ERROR} && result.task_id!==job.id))
            `uvm_fatal("TASK_IDENTITY","Result belongs to another channel or task")
        if(job.terminal_checked) `uvm_fatal("TASK_DUPLICATE","Task result checked more than once")
        if(result.outcome!==expected_outcome || result.code!==expected_code)
            `uvm_fatal("TASK_OUTCOME",$sformatf("channel=%0d task=%0d expected=%s/%0d actual=%s/%0d",
                channel_id,job.id,expected_outcome.name(),expected_code,result.outcome.name(),result.code))
        if(result.outcome==TASK_SUCCESS) begin
            if(result.value!==job.fence_value())
                `uvm_fatal("TASK_COMPLETION","Invalid Fence marker")
            check_copied_bytes(job,memory,job.expected.size());
        end
        if(result.outcome==TASK_ERROR && result.event_producer!==1)
            `uvm_fatal("TASK_ERROR_INFO","Error event producer is incorrect")
        if(result.outcome==TASK_ERROR) begin
            if($isunknown(result.prefix) || result.prefix>job.length)
                `uvm_fatal("TASK_ERROR_INFO","Committed prefix is unknown or exceeds task length")
            if(result.prefix!==job.expected_error_prefix && !(job.allow_zero_error_prefix && result.prefix===0))
                `uvm_fatal("TASK_ERROR_INFO","Committed prefix differs from the scenario expectation")
            check_copied_bytes(job,memory,int'(result.prefix));
        end
        if(result.outcome inside {TASK_FAULT,TASK_ABORTED} && result.cause!==0)
            `uvm_fatal("TASK_EVENT","Parked or discarded work raised a channel interrupt cause")
        if(result.outcome inside {TASK_FAULT,TASK_ERROR,TASK_FATAL,TASK_ABORTED})
            for(int i=0;i<job.fence_bytes();i++)
                if(memory.read_byte(job.fence_address+i)!==8'b0)
                    `uvm_fatal("TASK_COMPLETION","Incomplete task wrote its successor Fence memory")
        if(result.outcome inside {TASK_FAULT,TASK_FATAL,TASK_ABORTED} && result.event_producer!==0)
            `uvm_fatal("TASK_EVENT","Parked work, discard or failed report published an event")
        if(job.expect_payload_unchanged) foreach(job.initial_data[i])
            if(memory.read_byte(job.destination_byte(i))!==job.initial_data[i])
                `uvm_fatal("TASK_DATA","Rejected command changed payload memory")
        if(memory.read_byte(job.dest_address-1)!==8'ha6 || memory.read_byte(job.dest_address+job.length)!==8'h59)
            `uvm_fatal("TASK_GUARD","Destination boundary was overwritten")
        if(result.outcome!=TASK_FAULT) begin job.terminal_checked=1; checked_tasks++; end
    endfunction
    function void interrupt_result(pdma_result result);
        if(result.channel_id!==channel_id || $isunknown(result.cause) || result.cause==0)
            `uvm_fatal("TASK_IDENTITY","IRQ handler result has wrong channel or no cause")
        irq_service.sample(result.outcome);
    endfunction
endclass
