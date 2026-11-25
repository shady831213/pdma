class pdma_sg_range_test extends pdma_base_test;
    `uvm_component_utils(pdma_sg_range_test)
    typedef enum {OffsetPastEnd, StartWrap, EndWrap, ClippedTail} range_case;

    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        range_case kind;
        super.build_phase(phase); expected_cases=platform.channel_count*kind.num();
    endfunction
    virtual task run_scenario();
        range_case kind;
        foreach(env.device.channels[ch]) begin
            kind=kind.first();
            repeat(kind.num()) begin
                check_range(ch,kind);
                kind=kind.next();
            end
        end
    endtask
    local task check_range(int owner,range_case kind);
        pdma_task_flow flow_=env.device.task_flow(owner);
        pdma_sg_task job=new;
        job.dest_sg=0;
        job.expect_payload_unchanged=kind!=ClippedTail;
        flow_.open(job);
        // The descriptor's initial offset is one byte. A fully consumed
        // entry has no payload address; validate only the used part of a tail.
        case(kind)
            OffsetPastEnd: job.put_segment(env.memory,0,job.START,job.source_address,0);
            StartWrap: job.put_segment(env.memory,0,job.START,'1,2);
            EndWrap: job.put_segment(env.memory,0,job.START,64'hffff_ffff_ffff_fffd,4);
            ClippedTail: begin
                job.put_segment(env.memory,0,job.START,'1,1);
                job.put_segment(env.memory,0,job.EMPTY,job.address_at(0,0),job.source_first);
                job.put_segment(env.memory,0,job.SECOND,job.address_at(0,job.source_first),'1);
            end
        endcase
        if(kind!=ClippedTail) env.memory.release_regions(job.source_buffer.allocation.get_s_addr());
        flow_.submit();
        if(kind==ClippedTail) flow_.expect_fence();
        else begin
            flow_.expect_error(EVENT_CODE_INVALIDLENGTH);
            flow_.channel.reset();
        end
        flow_.close(); cases++;
    endtask
endclass
