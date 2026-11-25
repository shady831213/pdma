// Park a Copy while only its source segment is exhausted by the current span.
// A peer consumes both of its lists before software resumes the original owner.
class pdma_sg_parked_span_test extends pdma_base_test;
    `uvm_component_utils(pdma_sg_parked_span_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=platform.channel_count;
    endfunction
    virtual task run_scenario();
        foreach(env.device.channels[ch]) check_owner(ch);
    endtask
    local task check_owner(int owner);
        pdma_task_flow flow_=env.device.task_flow(owner);
        pdma_sg_task job=new;
        int unsigned beat=pdma_configuration::MEMORY_BEAT_BYTES;
        job.source_first=beat; job.dest_first=2*beat;
        flow_.open(job);
        inject(1,1,job.destination_byte(0));
        flow_.submit(); flow_.expect_fault();
        if(platform.channel_count>1) begin
            pdma_task_flow peer=env.device.task_flow((owner+1)%platform.channel_count);
            pdma_sg_task work=new;
            work.source_first=2*beat; work.dest_first=beat;
            peer.open(work); peer.submit(); peer.expect_fence(); peer.close();
        end
        // Only this first span was already read. A retry must use its saved
        // bytes, then fetch the remaining spans at their original boundaries.
        for(int i=0;i<job.source_first;i++) env.memory.write_byte(job.address_at(0,i),~job.expected[i]);
        platform.fault_enabled=0;
        flow_.channel.resume_task(); flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass
