class pdma_window_replay_test extends pdma_base_test;
    `uvm_component_utils(pdma_window_replay_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); platform.memory_outstanding=8;
        expected_cases=2*platform.channel_count;
    endfunction
    virtual task run_scenario();
        localparam int BURST=pdma_configuration::COPY_BURST_BYTES;
        for(int wr=0;wr<2;wr++) foreach(env.device.channels[ch]) begin
            pdma_task_flow flow_=env.device.task_flow(ch);
            pdma_task job=settings((2*`PDMA_COPY_SLOTS+1)*BURST);
            // Fault after at least one window of aligned payload, at a
            // seed-selected burst. Software uses only addresses and FaultStatus.
            int offset=(`PDMA_COPY_SLOTS+$urandom_range(`PDMA_COPY_SLOTS-1))*BURST;
            job.payload_alignment=pdma_configuration::AXI_BOUNDARY_BYTES;
            flow_.open(job);
            inject(wr,1,(wr ? job.dest_address : job.source_address)+offset);
            flow_.submit(); flow_.expect_fault();
            platform.fault_enabled=0;
            if(wr) env.memory.write_byte(job.source_address+offset,~job.expected[offset]);
            flow_.channel.resume_task(); flow_.expect_fence(); flow_.close(); cases++;
        end
    endtask
endclass
