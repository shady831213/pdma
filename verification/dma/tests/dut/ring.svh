class pdma_ring_wrap_test extends pdma_base_test;
    `uvm_component_utils(pdma_ring_wrap_test)
    function new(string name,uvm_component parent);
        super.new(name,parent);
    endfunction
    virtual task run_scenario();
        longint unsigned ring_address=env.device.driver.channels[0].command_address;
        repeat(3) begin
            pdma_task_flow flow_=env.device.task_flow();
            pdma_task job=settings(32);
            flow_.open(job); flow_.submit(); flow_.expect_fence(); flow_.close();
            if(env.device.driver.channels[0].command_address!=ring_address)
                `uvm_fatal("RING_LIFETIME","Task close replaced channel ring")
        end
        if(env.device.driver.channels[0].producer!=6) `uvm_fatal("RING_WRAP","Distinct tasks did not retain producer cursor")
        cases++;
    endtask
endclass

// A completed Fence releases only its task's allocations. The following
// stopping error leaves its own Fence permanently unconsumed until reset.
class pdma_same_channel_lifetime_test extends pdma_base_test;
    `uvm_component_utils(pdma_same_channel_lifetime_test)
    function new(string name,uvm_component parent);
        super.new(name,parent); expected_cases=2;
    endfunction
    virtual task run_scenario();
        pdma_task_flow first=env.device.task_flow(), second=env.device.task_flow();
        pdma_task rejected=settings(16);
        rejected.opcode=COMMANDHEADER_OPCODE_ATOMIC; rejected.expect_payload_unchanged=1;
        first.open(settings(32)); second.open(rejected);
        first.submit(); second.submit();
        second.expect_error(EVENT_CODE_UNSUPPORTED);
        first.expect_fence(); first.close(); cases++;
        second.channel.reset(); second.close(); cases++;
    endtask
endclass

// Exercise both split MMIO base writes and 64-bit AXI addresses on the wire.
// The inherited scenario fetches from the command ring, copies data, writes
// a Fence, then publishes a stopping error to the independently placed ring.
class pdma_high_ring_address_test extends pdma_same_channel_lifetime_test;
    `uvm_component_utils(pdma_high_ring_address_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        longint unsigned command_base,event_base;
        super.build_phase(phase);
        command_base={32'ha5a5_5a5a,platform.heap_base[31:0]};
        event_base={32'h5a5a_a5a5,platform.heap_base[31:0]};
        platform.command_ring_address=command_base;
        platform.event_ring_address[0]=event_base;
        platform.heap_size=command_base-platform.heap_base+platform.heap_size;
    endfunction
endclass
