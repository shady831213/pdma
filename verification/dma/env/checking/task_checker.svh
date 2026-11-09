class pdma_task_checker extends uvm_object;
    `uvm_object_utils(pdma_task_checker)
    int unsigned channel_id, checked_tasks;
    longint unsigned checked_bytes;
    function new(string name="task_checker"); super.new(name); endfunction
    virtual function void check(pdma_task job,pdma_result result,pdma_outcome expected_outcome,
        pdma_memory memory,int expected_code=0);
        if(result.channel_id!==channel_id || job.channel_id!==channel_id || result.task_id!==job.id)
            `uvm_fatal("TASK_IDENTITY","Result belongs to another channel or task")
        if(job.terminal_checked) `uvm_fatal("TASK_DUPLICATE","Task result checked more than once")
        if(result.outcome!==expected_outcome || result.code!==expected_code)
            `uvm_fatal("TASK_OUTCOME",$sformatf("channel=%0d task=%0d expected=%s/%0d actual=%s/%0d",
                channel_id,job.id,expected_outcome.name(),expected_code,result.outcome.name(),result.code))
        if(result.outcome==TASK_SUCCESS) begin
            if(result.value!==job.fence_value() || result.event_producer!==0 || result.cause!==0)
                `uvm_fatal("TASK_COMPLETION","Invalid Fence completion or unexpected event/cause")
            foreach(job.expected[i]) begin
                if(memory.read_byte(job.dest_address+i)!==job.expected[i])
                    `uvm_fatal("TASK_DATA",$sformatf("channel=%0d task=%0d destination byte=%0d",channel_id,job.id,i))
                checked_bytes++;
            end
        end
        if(result.outcome==TASK_ERROR && result.event_producer!==1)
            `uvm_fatal("TASK_ERROR_INFO","Error event producer is incorrect")
        if(result.outcome==TASK_ERROR) begin
            if($isunknown(result.prefix) || result.prefix>job.length)
                `uvm_fatal("TASK_ERROR_INFO","Committed prefix is unknown or exceeds task length")
            if(result.prefix!==job.expected_error_prefix)
                `uvm_fatal("TASK_ERROR_INFO","Committed prefix differs from the scenario expectation")
            for(int i=0;i<result.prefix;i++)
                if(memory.read_byte(job.dest_address+i)!==job.expected[i])
                    `uvm_fatal("TASK_DATA","Confirmed committed prefix does not match memory")
        end
        if(result.outcome inside {TASK_FAULT,TASK_ERROR} && result.value!==0)
            `uvm_fatal("TASK_COMPLETION","Incomplete task published its successor Fence")
        if(result.outcome inside {TASK_FAULT,TASK_ERROR})
            for(int i=0;i<job.fence_bytes();i++)
                if(memory.read_byte(job.fence_address+i)!==8'b0)
                    `uvm_fatal("TASK_COMPLETION","Incomplete task wrote its successor Fence memory")
        if(result.outcome inside {TASK_FAULT,TASK_FATAL} && result.event_producer!==0)
            `uvm_fatal("TASK_EVENT","Fault or failed report published an event")
        if(job.expect_payload_unchanged) foreach(job.initial_data[i])
            if(memory.read_byte(job.dest_address+i)!==job.initial_data[i])
                `uvm_fatal("TASK_DATA","Rejected command changed payload memory")
        if(memory.read_byte(job.dest_address-1)!==8'ha6 || memory.read_byte(job.dest_address+job.length)!==8'h59)
            `uvm_fatal("TASK_GUARD","Destination boundary was overwritten")
        if(result.outcome!=TASK_FAULT) begin job.terminal_checked=1; checked_tasks++; end
    endfunction
    function void interrupt_result(pdma_result result);
        if(result.channel_id!==channel_id || $isunknown(result.cause) || result.cause==0)
            `uvm_fatal("TASK_IDENTITY","IRQ handler result has wrong channel or no cause")
    endfunction
endclass
