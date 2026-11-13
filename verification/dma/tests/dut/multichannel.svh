// Independent scenario classes keep the regression parallelizable. Expectations
// use task identity/data and software-visible lifecycle, not RTL slot timing.
class pdma_multichannel_copy_test extends pdma_base_test;
    `uvm_component_utils(pdma_multichannel_copy_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); platform.memory_outstanding=8;
        expected_cases=platform.channel_count*3;
    endfunction
    virtual task run_scenario();
        repeat(3) begin
            pdma_task_flow flows[]=new[platform.channel_count];
            foreach(flows[i]) begin
                flows[i]=env.task_flow(i); flows[i].open(settings(0,1)); flows[i].submit();
            end
            foreach(flows[i]) begin flows[i].expect_result(TASK_SUCCESS); flows[i].close(); cases++; end
        end
    endtask
endclass

class pdma_multichannel_park_test extends pdma_base_test;
    `uvm_component_utils(pdma_multichannel_park_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); platform.memory_outstanding=8;
        expected_cases=platform.channel_count+1;
    endfunction
    virtual task run_scenario();
        pdma_task_flow flows[]=new[platform.channel_count];
        // Park every owner except the final peer, retaining a full write burst.
        foreach(flows[i]) begin
            pdma_task job=settings(64); job.payload_alignment=64;
            flows[i]=env.task_flow(i); flows[i].open(job);
            if(i+1<flows.size()) inject(1,1,job.dest_address);
            else platform.fault_enabled=0;
            flows[i].submit();
            flows[i].expect_result(i+1<flows.size() ? TASK_FAULT : TASK_SUCCESS);
        end
        platform.fault_enabled=0;
        if(flows.size()>1) begin
            flows[0].channel.request_reset(); flows[0].cancel(); cases++;
        end else begin flows[0].close(); cases++; end
        // No source reread is permitted for the retained successful reads.
        for(int i=1;i+1<flows.size();i++) begin
            foreach(flows[i].job.expected[b]) env.memory.write_byte(flows[i].job.source_address+b,~flows[i].job.expected[b]);
            flows[i].channel.resume_task();
            flows[i].channel.await_reg(flows[i].channel.registers.control_block.recovery_block.fault_status_reg,1,0);
            flows[i].expect_result(TASK_SUCCESS); flows[i].close(); cases++;
        end
        if(flows.size()>1) begin flows[flows.size()-1].close(); cases++; end
        flows[0]=env.task_flow(0); flows[0].open(settings(128)); flows[0].submit();
        flows[0].expect_result(TASK_SUCCESS); flows[0].close(); cases++;
    endtask
endclass

class pdma_multichannel_reset_test extends pdma_base_test;
    `uvm_component_utils(pdma_multichannel_reset_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); platform.memory_outstanding=8;
        expected_cases=platform.channel_count+1;
    endfunction
    virtual task run_scenario();
        pdma_task_flow flows[]=new[platform.channel_count];
        bit[31:0] busy;
        foreach(flows[i]) begin flows[i]=env.task_flow(i); flows[i].open(settings(512)); end
        platform.hold_write=1; flows[0].submit();
        wait(platform.held.size()!=0);
        flows[0].channel.request_reset();
        flows[0].channel.read_reg(flows[0].channel.admin.reset_reg,busy);
        if(!busy[0]) `uvm_fatal("RESET_DRAIN","Target reset completed with an outstanding write")
        // Queue peers while the old owner's responses are still retained. Shared
        // AXI ID ordering can backpressure them, but must never misroute receipts.
        for(int i=1;i<flows.size();i++) flows[i].submit();
        platform.hold_write=0;
        foreach(platform.held[i]) platform.held[i].suspend_response=0;
        platform.held.delete(); flows[0].cancel(); cases++;
        for(int i=1;i<flows.size();i++) begin flows[i].expect_result(TASK_SUCCESS); flows[i].close(); cases++; end
        flows[0]=env.task_flow(0); flows[0].open(settings(64)); flows[0].submit();
        flows[0].expect_result(TASK_SUCCESS); flows[0].close(); cases++;
    endtask
endclass

class pdma_multichannel_irq_test extends pdma_base_test;
    `uvm_component_utils(pdma_multichannel_irq_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=platform.channel_count;
    endfunction
    virtual task run_scenario();
        pdma_task_flow flows[]=new[platform.channel_count];
        foreach(flows[i]) begin
            pdma_task job=settings(16); job.opcode=COMMANDHEADER_OPCODE_TRAP; job.expect_payload_unchanged=1;
            flows[i]=env.task_flow(i); flows[i].open(job);
        end
        // Channel notification enables must not depend on the global-fatal mask.
        env.device.write_reg(env.device.registers.irq_enable_reg,0);
        foreach(flows[i]) flows[i].submit();
        @(negedge env.control.core_clk); env.control.irq_ready='1;
        foreach(flows[i]) begin
            flows[i].expect_interrupt(TASK_ERROR,EVENT_CODE_UNSUPPORTED);
            if(flows[i].channel.delivered!=1) `uvm_fatal("CHANNEL_IRQ_COUNT","Wrong channel interrupt count")
            flows[i].close(); cases++;
        end
        @(negedge env.control.core_clk); env.control.irq_ready=0;
        if(env.device_handler.delivered!=0) `uvm_fatal("CHANNEL_IRQ_SCOPE","Channel error delivered as device fatal")
    endtask
endclass

class pdma_multichannel_throughput_test extends pdma_throughput_test;
    `uvm_component_utils(pdma_multichannel_throughput_test)
    pdma_performance metrics[];
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=platform.channel_count;
        metrics=new[platform.channel_count]; metrics[0]=performance;
        for(int i=1;i<metrics.size();i++) metrics[i]=new($sformatf("performance_%0d",i),this);
    endfunction
    virtual function void connect_phase(uvm_phase phase);
        super.connect_phase(phase);
        for(int i=1;i<metrics.size();i++) metrics[i].attach(env.memory_bus.slave[0].monitor);
    endfunction
    virtual task run_scenario();
        pdma_task_flow flows[]=new[platform.channel_count];
        foreach(flows[i]) begin
            pdma_task job=settings(transfer_bytes); job.payload_alignment=pdma_configuration::AXI_BOUNDARY_BYTES;
            flows[i]=env.task_flow(i); flows[i].open(job);
            metrics[i].begin_task(job,clocks.period_ns[2]*1ns,flows[i].channel.next_command_address());
            flows[i].submit();
        end
        foreach(flows[i]) begin
            flows[i].expect_result(TASK_SUCCESS); metrics[i].finish(write_latency_budget());
            flows[i].close(); cases++;
        end
    endtask
endclass
