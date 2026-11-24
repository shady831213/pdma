class pdma_task_flow extends uvm_object;
    `uvm_object_utils(pdma_task_flow)
    pdma_task job;
    pdma_channel channel;
    jvs_irq_handler irq_handler;
    pdma_device device;
    pdma_task_checker checker_;
    pdma_memory memory;
    jvs_memory_allocator allocator;
    uvm_analysis_port#(pdma_observation) observations;
    bit published;
    pdma_outcome terminal_outcome;
    function new(string name="task_flow"); super.new(name); endfunction
    task open(pdma_task settings);
        job=settings; job.channel_id=channel.channel_id;
        job.id=channel.next_id; channel.next_id+=job.trap_after_fence ? 3 : 2;
        if(!channel.configured) channel.configure();
        job.allocate(allocator,memory);
    endtask
    virtual task submit();
        channel.submit(job); published=1;
    endtask
    function void publish_observation(pdma_result result);
        pdma_observation item=new;
        item.channel_id=job.channel_id; item.task_id=job.id; item.outcome=result.outcome;
        item.length=job.length;
        item.source_lane=job.source_address%pdma_configuration::MEMORY_BEAT_BYTES;
        item.dest_lane=job.dest_address%pdma_configuration::MEMORY_BEAT_BYTES;
        item.cross_page=(job.source_address%pdma_configuration::AXI_BOUNDARY_BYTES+job.length>pdma_configuration::AXI_BOUNDARY_BYTES) ||
            (job.dest_address%pdma_configuration::AXI_BOUNDARY_BYTES+job.length>pdma_configuration::AXI_BOUNDARY_BYTES);
        if(result.outcome==TASK_ERROR) begin
            item.event_cross_page=result.event_address%pdma_configuration::AXI_BOUNDARY_BYTES+EVENT_SIZE>pdma_configuration::AXI_BOUNDARY_BYTES;
        end
        item.fence_wide=job.fence_wide;
        item.latency=$realtime-job.submitted_at;
        observations.write(item);
    endfunction
    local function void check_result(pdma_result result,pdma_outcome expected_outcome,int code=0);
        checker_.check(job,result,expected_outcome,memory,code);
        terminal_outcome=result.outcome;
        publish_observation(result);
    endfunction
    task expect_fence();
        pdma_result result=new;
        result.channel_id=channel.channel_id; result.outcome=TASK_SUCCESS;
        repeat(channel.cfg.poll_limit) begin
            result.value=job.read_fence(memory);
            if(result.value!==0) begin
                result.task_id=result.value;
                check_result(result,TASK_SUCCESS);
                return;
            end
            #(channel.cfg.fence_poll_interval);
        end
        `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d task=%0d Fence marker did not arrive",channel.channel_id,job.id))
    endtask
    task expect_fault();
        FaultStatus status_reg=channel.registers.control_block.recovery_block.fault_status_reg;
        pdma_result result=new;
        uvm_reg_data_t mask=1<<status_reg.faulted_field.get_lsb_pos();
        channel.await_reg(status_reg,mask,mask);
        result.channel_id=channel.channel_id; result.outcome=TASK_FAULT;
        channel.read_reg(channel.registers.event_block.producer_reg,result.event_producer);
        channel.read_reg(channel.registers.control_block.irq_block.cause_reg,result.cause);
        check_result(result,TASK_FAULT);
    endtask
    task expect_error(int code);
        pdma_result result=new;
        bit available;
        result.channel_id=channel.channel_id;
        repeat(channel.cfg.poll_limit) begin
            channel.pop_event(result,available);
            if(available) begin check_result(result,TASK_ERROR,code); return; end
        end
        `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d task=%0d error event did not arrive",channel.channel_id,job.id))
    endtask
    task expect_fatal(int code);
        pdma_result result=new;
        result.channel_id=channel.channel_id; result.outcome=TASK_FATAL;
        repeat(channel.cfg.poll_limit) begin
            channel.read_reg(channel.registers.control_block.recovery_block.fatal_reg,result.code);
            if(result.code!=0) begin
                channel.read_reg(channel.registers.event_block.producer_reg,result.event_producer);
                check_result(result,TASK_FATAL,code);
                return;
            end
        end
        `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d task=%0d channel fatal did not arrive",channel.channel_id,job.id))
    endtask
    task wait_irq_result(output pdma_result result);
        jvs_irq_result_trans irq;
        irq_handler.get_irq_result(irq,channel.cfg.irq_timeout_ns);
        if(irq==null)
            `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d IRQ result did not arrive",channel.channel_id))
        else if(!$cast(result,irq.attr) || result==null)
            `uvm_fatal("TASK_IDENTITY","Missing PDMA observation in IRQ result")
    endtask
    task expect_interrupt(pdma_outcome outcome,int code=0);
        pdma_result result;
        wait_irq_result(result);
        checker_.interrupt_result(result);
        if(result.outcome==IRQ_NOTIFICATION && outcome==TASK_SUCCESS && job.trap_after_fence) begin
            if(result.cause!==(32'b1<<pdma_rtl_ChannelIrqCauseLayout::VALUE_TRAP_LSB))
                `uvm_fatal("TASK_COMPLETION","Expected the Trap cause without an error event")
            // The IRQ identifies the channel. Its preceding Fence supplies the task identity.
            result.value=job.read_fence(memory);
            result.task_id=result.value;
            result.outcome=TASK_SUCCESS;
        end
        check_result(result,outcome,code);
    endtask
    task close();
        if(published && !job.terminal_checked)
            `uvm_fatal("TASK_LIFETIME","Flow attempted cleanup before checking its terminal result")
        if(published && terminal_outcome!=TASK_SUCCESS && channel.configured)
            `uvm_fatal("TASK_LIFETIME","Stopped work requires a completed reset before buffer release")
        job.deallocate(allocator,memory); published=0;
    endtask
    task cancel(bit global_scope=0);
        pdma_result result=new;
        if(global_scope) device.finish_reset();
        else channel.finish_reset();
        result.channel_id=channel.channel_id; result.outcome=TASK_RESET;
        checker_.check(job,result,TASK_RESET,memory);
        publish_observation(result);
        job.deallocate(allocator,memory); published=0;
    endtask
endclass
