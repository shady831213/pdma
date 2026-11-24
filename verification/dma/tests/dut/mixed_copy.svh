// Every channel runs every endpoint pair with both Fence polling and Trap
// delivery. The seed shuffles their order and the independent SG split points.
class pdma_mixed_copy_test extends pdma_base_test;
    `uvm_component_utils(pdma_mixed_copy_test)
    localparam int BEAT=pdma_configuration::MEMORY_BEAT_BYTES;
    localparam int BURST=pdma_configuration::COPY_BURST_BYTES;
    typedef bit[2:0] mode_t; // {use Trap, SG destination, SG source}
    localparam int MODES=1<<$bits(mode_t);
    function new(string name,uvm_component parent);
        super.new(name,parent); interrupt_driven=1;
    endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        platform.memory_outstanding=8; platform.irq_timeout_ns*=platform.channel_count;
        expected_cases=MODES*platform.channel_count;
    endfunction
    local function pdma_task make_task(mode_t mode);
        pdma_sg_task job=new;
        job.source_sg=mode[0]; job.dest_sg=mode[1]; job.trap_after_fence=mode[2];
        job.max_length=2*BURST+1;
        if(!job.randomize() with {
            length dist {BEAT-1:=1, BEAT:=1, BEAT+1:=1, BURST-1:=1, BURST:=1, BURST+1:=1,
                [2:2*BURST+1]:/6};
        }) `uvm_fatal("MIXED_CONFIG","Cannot generate a legal Copy")
        return job;
    endfunction
    virtual task run_scenario();
        mode_t modes[`PDMA_CHANNEL_COUNT][MODES];
        int order[$];
        foreach(modes[ch]) begin
            mode_t deck[MODES];
            foreach(deck[r]) deck[r]=mode_t'(r);
            deck.shuffle(); modes[ch]=deck; order.push_back(ch);
        end
        for(int round=0;round<MODES;round++) begin
            pdma_task_flow flows[`PDMA_CHANNEL_COUNT];
            order.shuffle();
            foreach(order[n]) begin
                int ch=order[n];
                flows[ch]=env.device.task_flow(ch);
                flows[ch].open(make_task(modes[ch][round])); flows[ch].submit();
            end
            order.shuffle();
            foreach(order[n]) begin
                pdma_task_flow f=flows[order[n]];
                if(f.job.trap_after_fence) f.expect_interrupt(TASK_SUCCESS);
                else f.expect_fence();
                f.close(); cases++;
            end
        end
    endtask
endclass
