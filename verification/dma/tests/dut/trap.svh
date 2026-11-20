// Submit a single batch per channel so software timing cannot order Fence and Trap.
class pdma_trap_completion_test extends pdma_base_test;
    `uvm_component_utils(pdma_trap_completion_test)
    function new(string name,uvm_component parent); super.new(name,parent); interrupt_driven=1; endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase);
        expected_cases=2*platform.channel_count;
        // One shared engine can run each peer's byte-wise Copy before this Trap.
        platform.irq_timeout_cycles*=platform.channel_count;
    endfunction
    virtual task run_scenario();
        pdma_task_flow flows[]=new[platform.channel_count];
        foreach(flows[i]) begin
            ChannelIrqMask mask_reg;
            flows[i]=env.task_flow(i);
            mask_reg=flows[i].channel.registers.control_block.irq_block.mask_reg;
            flows[i].channel.write_reg(mask_reg,1<<mask_reg.event_non_empty_field.get_lsb_pos());
        end
        for(int round=0;round<2;round++) begin
            foreach(flows[i]) begin
                pdma_task job=settings(64,1);
                job.trap_after_fence=1;
                flows[i].open(job); flows[i].submit();
            end
            foreach(flows[i]) begin
                flows[i].expect_interrupt(TASK_SUCCESS);
                flows[i].close(); cases++;
            end
        end
        foreach(flows[i]) begin
            pdma_channel ch=flows[i].channel;
            ChannelInterrupts irq=ch.registers.control_block.irq_block;
            uvm_reg_data_t cause;
            ch.read_reg(irq.cause_reg,cause);
            if(cause!==0 || ch.delivered!=2 || env.control.irq[i]!==0)
                `uvm_fatal("TRAP_ACK","Expected one service and W1C per batch")
        end
    endtask
endclass

class pdma_trap_busy_handler_test extends pdma_base_test;
    `uvm_component_utils(pdma_trap_busy_handler_test)
    function new(string name,uvm_component parent); super.new(name,parent); interrupt_driven=1; endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=2;
    endfunction
    virtual task run_scenario();
        pdma_task_flow flows[2];
        pdma_channel ch=env.device.channels[0];
        ch.isr_service.get();
        foreach(flows[i]) begin
            pdma_task job=settings(16);
            job.trap_after_fence=1;
            flows[i]=env.task_flow(0);
            flows[i].open(job); flows[i].submit(); ch.await_consumed();
            flows[i].expect_result(TASK_SUCCESS); flows[i].close(); cases++;
            if(env.control.irq[0]!==1 || ch.delivered!=0)
                `uvm_fatal("TRAP_BUSY_HANDLER","Expected two completed batches to share a held IRQ")
        end
        ch.isr_service.put();
        begin
            pdma_result wake;
            ch.wait_interrupt(wake); flows[0].checker_.interrupt_result(wake);
        end
        if(ch.delivered!=1 || env.control.irq[0]!==0) `uvm_fatal("TRAP_IRQ_COUNT","One ACK must clear both Traps")
    endtask
endclass

class pdma_trap_mask_test extends pdma_base_test;
    `uvm_component_utils(pdma_trap_mask_test)
    function new(string name,uvm_component parent); super.new(name,parent); interrupt_driven=1; endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=2;
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_channel ch=flow_.channel;
        ChannelInterrupts irq=ch.registers.control_block.irq_block;
        // Hold software service while observing the physical line.
        for(int masked=0;masked<2;masked++) begin
            pdma_task job=settings();
            job.trap_after_fence=1;
            ch.isr_service.get();
            flow_.open(job);
            if(masked) ch.write_reg(irq.mask_reg,1<<irq.mask_reg.trap_field.get_lsb_pos());
            else ch.write_reg(irq.enable_reg,0);
            flow_.submit(); ch.await_consumed();
            if(env.control.irq[0]!==0) `uvm_fatal("TRAP_MASK","Masked Trap asserted IRQ")
            if(masked) ch.write_reg(irq.mask_reg,0);
            else ch.write_reg(irq.enable_reg,1<<irq.enable_reg.enabled_field.get_lsb_pos());
            ch.write_reg(irq.trap_ack_reg,0);
            if(env.control.irq[0]!==1) `uvm_fatal("TRAP_UNMASK","Enabled sticky Trap must assert until W1C")
            ch.isr_service.put();
            flow_.expect_interrupt(TASK_SUCCESS); flow_.close(); cases++;
        end
    endtask
endclass

class pdma_trap_pending_test extends pdma_base_test;
    `uvm_component_utils(pdma_trap_pending_test)
    function new(string name,uvm_component parent); super.new(name,parent); interrupt_driven=1; endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=1;
    endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.task_flow();
        pdma_channel ch=flow_.channel;
        ChannelInterrupts irq=ch.registers.control_block.irq_block;
        pdma_task job=settings();
        job.trap_after_fence=1;
        ch.isr_service.get();
        flow_.open(job); flow_.submit(); ch.await_consumed();
        flow_.expect_result(TASK_SUCCESS); flow_.close(); cases++;
        if(env.control.irq[0]!==1) `uvm_fatal("TRAP_LEVEL","Missing retained IRQ before reset")
        ch.request_reset(); ch.finish_reset();
        begin
            uvm_reg_data_t cause;
            ch.read_reg(irq.cause_reg,cause);
            if(cause!==0 || env.control.irq[0]!==0) `uvm_fatal("TRAP_RESET","Reset retained Trap")
        end
        ch.isr_service.put();
    endtask
endclass
