class pdma_channel extends pdma_register_access;
    int unsigned channel_id, producer;
    longint unsigned next_id=1;
    pdma_memory memory;
    ChannelBlock registers;
    ChannelAdmin admin;
    int unsigned delivered;
    longint unsigned command_address, event_address;
    jvs_memory_acc_block command_allocation, event_allocation;
    jvs_memory_allocator allocator;
    int unsigned event_consumer;
    semaphore isr_service=new(1);
    semaphore producer_lock=new(1), event_lock=new(1);
    bit configured, interrupt_driven;
    function new(string name="channel"); super.new(name); endfunction
    function void bind_registers(DmaRegisters root,int index);
        channel_id=index;
        registers=root.channels_block[index];
        admin=root.admin_block.channels_block[index];
    endfunction
    task setup();
        allocator.malloc(command_allocation,cfg.command_capacity*COMMAND_SIZE,
            e_alignment'($clog2(COMMAND_ALIGNMENT)),channel_id==0 ? cfg.command_ring_address : `JVS_MEM_NULL);
        allocator.malloc(event_allocation,cfg.event_capacity*EVENT_SIZE,
            e_alignment'($clog2(EVENT_ALIGNMENT)),cfg.event_ring_address.exists(channel_id)
                ? cfg.event_ring_address[channel_id] : `JVS_MEM_NULL);
        if(command_allocation==null || event_allocation==null) `uvm_fatal("RING_ALLOCATION","Channel ring allocation failed")
        command_address=command_allocation.get_s_addr(); event_address=event_allocation.get_s_addr();
        memory.permit(command_address,command_address,cfg.command_capacity*COMMAND_SIZE,cfg.ring_context,0,channel_id);
        memory.permit(event_address,event_address,cfg.event_capacity*EVENT_SIZE,cfg.ring_context,1,channel_id);
        configure();
    endtask
    task configure();
        producer=0; event_consumer=0;
        write_reg(registers.command_block.base_reg,command_address);
        write_reg(registers.command_block.capacity_reg,cfg.command_capacity);
        write_reg(registers.event_block.base_reg,event_address);
        write_reg(registers.event_block.capacity_reg,cfg.event_capacity);
        write_reg(registers.control_block.config_block.ring_context_reg,cfg.ring_context);
        write_reg(registers.control_block.irq_block.enable_reg,
            int'(interrupt_driven)<<registers.control_block.irq_block.enable_reg.enabled_field.get_lsb_pos());
        write_reg(registers.control_block.config_block.enable_reg,
            1<<registers.control_block.config_block.enable_reg.enabled_field.get_lsb_pos());
        configured=1;
    endtask
    function longint unsigned next_command_address();
        return command_address+(producer%cfg.command_capacity)*COMMAND_SIZE;
    endfunction
    function void publish_record(Command_t record_value,int unsigned cursor);
        longint unsigned address=command_address+(cursor%cfg.command_capacity)*COMMAND_SIZE;
        for(int i=0;i<COMMAND_SIZE;i++) memory.write_byte(address+i,record_value[i*8+:8]);
    endfunction
    task submit(pdma_task job,bit publish_fence=1);
        bit[31:0] consumer;
        int unsigned count=1+int'(publish_fence)+int'(publish_fence && job.trap_after_fence);
        producer_lock.get();
        if(!configured) `uvm_fatal("CHANNEL_LIFETIME","Submission requires configured channel")
        read_reg(registers.command_block.consumer_reg,consumer);
        if(producer-consumer+count>cfg.command_capacity)
            `uvm_fatal("COMMAND_RING_FULL","No command ring capacity for task")
        publish_record(job.command_record(),producer);
        if(publish_fence) publish_record(job.fence_record(),producer+1);
        if(publish_fence && job.trap_after_fence) publish_record(job.trap_record(),producer+2);
        job.submitted_at=$realtime;
        producer+=count;
        write_reg(registers.command_block.producer_reg,producer);
        producer_lock.put();
    endtask
    task await_consumed();
        await_reg(registers.command_block.consumer_reg,'1,producer);
    endtask
    task teardown();
        reset();
        memory.release_regions(command_address); memory.release_regions(event_address);
        allocator.free(command_allocation); allocator.free(event_allocation);
    endtask
    task pop_event(pdma_result result,output bit available);
        bit[31:0] published;
        Event_t event_value='0;
        ErrorInfo_t info;
        longint unsigned address;
        event_lock.get();
        read_reg(registers.event_block.producer_reg,published);
        result.event_producer=published;
        available=published!=event_consumer;
        if(available) begin
            address=event_address+(event_consumer%cfg.event_capacity)*EVENT_SIZE;
            for(int i=0;i<EVENT_SIZE;i++) event_value[i*8+:8]=memory.read_byte(address+i);
            result.outcome=TASK_ERROR; result.task_id=Event_get_id(event_value);
            result.code=Event_get_code(event_value); info=Event_get_info(event_value);
            result.prefix=ErrorInfo_get_bytes_completed(info); result.event_address=address;
            event_consumer++;
            write_reg(registers.event_block.consumer_reg,event_consumer);
        end
        event_lock.put();
    endtask
    task request_reset();
        write_reg(admin.reset_reg,1<<admin.reset_reg.request_field.get_lsb_pos());
    endtask
    // Called only after this owner's reset or a completed device-wide reset.
    function void invalidate_after_reset();
        producer=0; event_consumer=0; configured=0;
    endfunction
    task finish_reset();
        await_reg(admin.reset_reg,1<<admin.reset_reg.request_field.get_lsb_pos(),0);
        invalidate_after_reset();
    endtask
    task reset();
        request_reset(); finish_reset();
    endtask
    task resume_task();
        write_reg(registers.control_block.recovery_block.resume_reg,
            1<<registers.control_block.recovery_block.resume_reg.request_field.get_lsb_pos());
    endtask
    task service_irq(output pdma_result result);
        uvm_reg_data_t fatal_code;
        bit available;
        isr_service.get();
        result=new; result.channel_id=channel_id;
        read_reg(registers.control_block.irq_block.cause_reg,result.cause);
        result.outcome=IRQ_NOTIFICATION;
        // Dispatch from the cause snapshot. A Trap-only ISR must leave a
        // subsequently published diagnostic for the next interrupt service.
        if(result.cause[registers.control_block.irq_block.cause_reg.error_field.get_lsb_pos()] ||
            result.cause[registers.control_block.irq_block.cause_reg.event_non_empty_field.get_lsb_pos()] ||
            result.cause[registers.control_block.irq_block.cause_reg.channel_fatal_field.get_lsb_pos()]) begin
            // A stopping error can assert before the event write completes. Only
            // one diagnostic is produced per stop, including when software read it
            // before unmasking the retained IRQ. Reset clears the consumer cursor.
            for(int n=0;n<cfg.poll_limit;n++) begin
                pop_event(result,available);
                if(available) break;
                read_reg(registers.control_block.recovery_block.fatal_reg,fatal_code);
                result.code=fatal_code;
                result.outcome=fatal_code!=0 ? TASK_FATAL : IRQ_NOTIFICATION;
                if(fatal_code!=0 || event_consumer!=0 ||
                    !result.cause[registers.control_block.irq_block.cause_reg.error_field.get_lsb_pos()]) break;
                if(n+1==cfg.poll_limit)
                    `uvm_fatal("ERROR_REPORT_TIMEOUT","Stopped channel has no event or fatal status")
            end
        end
        if(result.cause[registers.control_block.irq_block.cause_reg.trap_field.get_lsb_pos()])
            write_reg(registers.control_block.irq_block.trap_ack_reg,1);
        if(result.cause[registers.control_block.irq_block.cause_reg.channel_fatal_field.get_lsb_pos()] ||
            result.cause[registers.control_block.irq_block.cause_reg.error_field.get_lsb_pos()])
            write_reg(registers.control_block.irq_block.enable_reg,0);
        isr_service.put();
        delivered++;
    endtask
endclass
