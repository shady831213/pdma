class pdma_negative_checker extends pdma_task_checker;
    `uvm_object_utils(pdma_negative_checker)
    string mutation;
    function new(string name="negative_checker");
        super.new(name); void'($value$plusargs("checker_mutation=%s",mutation));
    endfunction
    virtual function void check(pdma_task job,pdma_result result,pdma_outcome expected_outcome,
        pdma_memory memory,int expected_code=0);
        if(mutation=="data") memory.write_byte(job.dest_address,~job.expected[0]);
        if(mutation=="identity") result.channel_id++;
        if(mutation=="unknown_identity") result.task_id='x;
        if(mutation=="early_fence") memory.write_byte(job.fence_address,1);
        if(mutation=="underreported_prefix") result.prefix=0;
        super.check(job,result,expected_outcome,memory,expected_code);
        if(mutation=="duplicate_completion") super.check(job,result,expected_outcome,memory,expected_code);
    endfunction
endclass

class pdma_missing_fence extends pdma_task_flow;
    `uvm_object_utils(pdma_missing_fence)
    function new(string name="missing_fence"); super.new(name); endfunction
    virtual task submit(); channel.submit(job,0); published=1; endtask
endclass
