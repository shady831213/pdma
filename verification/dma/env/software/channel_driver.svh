class pdma_channel extends uvm_object;
    int unsigned channel_id, vector, producer;
    longint unsigned next_id=1;
    pdma_memory memory;
    virtual pdma_control_if control;
    ChannelBlock registers;
    ChannelAdmin admin;
    DmaControl dma_control;
    mailbox#(pdma_result) interrupts=new;
    int unsigned delivered;
    longint unsigned command_address, event_address;
    jvs_memory_acc_block command_allocation, event_allocation;
    jvs_memory_allocator allocator;
    pdma_configuration cfg;
    int unsigned event_consumer;
    semaphore producer_lock=new(1), event_lock=new(1);
    bit configured;
    function new(string name="channel"); super.new(name); endfunction
    function void bind_registers(DmaRegisters root,int index,int irq_vector);
        channel_id=index; vector=irq_vector;
        registers=root.channels_block[index];
        admin=root.admin_block.channels_block[index];
        dma_control=root.admin_block.control_block;
    endfunction
    task write_reg(uvm_reg register_,uvm_reg_data_t value);
        uvm_status_e status;
        register_.write(status,value,UVM_FRONTDOOR);
        if(status!=UVM_IS_OK)
            `uvm_fatal("MMIO_RESPONSE",$sformatf("Rejected register write: %s",register_.get_full_name()))
    endtask
    task read_reg(uvm_reg register_,output uvm_reg_data_t value);
        uvm_status_e status;
        register_.read(status,value,UVM_FRONTDOOR);
        if(status!=UVM_IS_OK)
            `uvm_fatal("MMIO_RESPONSE",$sformatf("Rejected register read: %s",register_.get_full_name()))
    endtask
    task await_reg(uvm_reg register_,uvm_reg_data_t mask,uvm_reg_data_t expected);
        uvm_reg_data_t value;
        for(int n=0;n<cfg.poll_limit;n++) begin
            read_reg(register_,value);
            if((value & mask)==expected) return;
        end
        `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d register=%s expected=%h observed=%h",channel_id,register_.get_full_name(),expected,value))
    endtask
    task setup();
        allocator.malloc(command_allocation,cfg.command_capacity*COMMAND_SIZE,
            e_alignment'($clog2(COMMAND_ALIGNMENT)),cfg.command_ring_address);
        allocator.malloc(event_allocation,cfg.event_capacity*EVENT_SIZE,
            e_alignment'($clog2(EVENT_ALIGNMENT)),cfg.event_ring_address);
        if(command_allocation==null || event_allocation==null) `uvm_fatal("RING_ALLOCATION","Channel ring allocation failed")
        command_address=command_allocation.get_s_addr(); event_address=event_allocation.get_s_addr();
        memory.permit(command_address,command_address,cfg.command_capacity*COMMAND_SIZE,cfg.ring_context,0);
        memory.permit(event_address,event_address,cfg.event_capacity*EVENT_SIZE,cfg.ring_context,1);
        configure();
    endtask
    task configure();
        producer=0; event_consumer=0;
        write_reg(registers.command_block.base_reg,command_address);
        write_reg(registers.command_block.capacity_reg,cfg.command_capacity);
        write_reg(registers.event_block.base_reg,event_address);
        write_reg(registers.event_block.capacity_reg,cfg.event_capacity);
        write_reg(registers.control_block.config_block.ring_context_reg,cfg.ring_context);
        write_reg(dma_control.irq_enable_reg,1<<dma_control.irq_enable_reg.enabled_field.get_lsb_pos());
        write_reg(registers.control_block.irq_block.enable_reg,
            1<<registers.control_block.irq_block.enable_reg.enabled_field.get_lsb_pos());
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
        producer_lock.get();
        if(!configured) `uvm_fatal("CHANNEL_LIFETIME","Submission requires configured channel")
        read_reg(registers.command_block.consumer_reg,consumer);
        if(producer-consumer+(publish_fence ? 2 : 1)>cfg.command_capacity)
            `uvm_fatal("COMMAND_RING_FULL","No command ring capacity for task")
        publish_record(job.copy_record(),producer);
        if(publish_fence) publish_record(job.fence_record(),producer+1);
        job.submitted_at=$realtime;
        producer+=publish_fence ? 2 : 1;
        write_reg(registers.command_block.producer_reg,producer);
        producer_lock.put();
    endtask
    task await_consumed();
        await_reg(registers.command_block.consumer_reg,'1,producer);
    endtask
    task teardown();
        request_reset(); finish_reset();
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
        available=published!=event_consumer;
        if(available) begin
            address=event_address+(event_consumer%cfg.event_capacity)*EVENT_SIZE;
            for(int i=0;i<EVENT_SIZE;i++) event_value[i*8+:8]=memory.read_byte(address+i);
            result.outcome=TASK_ERROR; result.task_id=Event_get_id(event_value);
            result.code=Event_get_code(event_value); info=Event_get_info(event_value);
            result.prefix=ErrorInfo_get_bytes_completed(info); result.event_producer=published;
            event_consumer++;
            write_reg(registers.event_block.consumer_reg,event_consumer);
        end
        event_lock.put();
    endtask
    task request_reset(bit global_scope=0);
        Reset reset_reg=global_scope ? dma_control.reset_reg : admin.reset_reg;
        write_reg(reset_reg,1<<reset_reg.request_field.get_lsb_pos());
    endtask
    task finish_reset(bit global_scope=0);
        Reset reset_reg=global_scope ? dma_control.reset_reg : admin.reset_reg;
        await_reg(reset_reg,1<<reset_reg.request_field.get_lsb_pos(),0);
        producer=0; event_consumer=0; configured=0;
    endtask
    task resume_task();
        write_reg(registers.control_block.recovery_block.resume_reg,
            1<<registers.control_block.recovery_block.resume_reg.request_field.get_lsb_pos());
    endtask
    task observe(pdma_task job,output pdma_result result);
        bit[31:0] fatal_code, faulted, producer_value, cause;
        longint unsigned value;
        bit available;
        result=null;
        read_reg(registers.control_block.recovery_block.fatal_reg,fatal_code);
        read_reg(registers.control_block.recovery_block.fault_status_reg,faulted);
        read_reg(registers.event_block.producer_reg,producer_value);
        read_reg(registers.control_block.irq_block.cause_reg,cause);
        value=job.fence_wide ? memory.read_long(job.fence_address) : memory.read_int(job.fence_address);
        if(fatal_code!=0 || producer_value!=event_consumer || faulted!=0 || value!=0) begin
            result=new; result.channel_id=channel_id; result.task_id=job.id;
            result.cause=cause; result.event_producer=producer_value; result.value=value;
            if(fatal_code!=0) begin result.outcome=TASK_FATAL; result.code=fatal_code; end
            else if(producer_value!=event_consumer) begin
                pop_event(result,available);
                if(!available) result=null;
            end else if(faulted!=0) result.outcome=TASK_FAULT;
            else result.outcome=TASK_SUCCESS;
        end
    endtask
    task wait_result(pdma_task job,output pdma_result result);
        for(int n=0;n<cfg.poll_limit;n++) begin
            observe(job,result);
            if(result!=null) return;
        end
        `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d task=%0d has no completion",channel_id,job.id))
    endtask
    task wait_interrupt(output pdma_result result);
        fork
            interrupts.get(result);
            begin
                repeat(cfg.irq_timeout_cycles) @(posedge control.core_clk);
                `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d IRQ did not arrive",channel_id))
            end
        join_any
        disable fork;
    endtask
    task interrupt_snapshot(output pdma_result result);
        bit[31:0] fatal_code;
        bit available;
        result=new; result.channel_id=channel_id;
        read_reg(registers.control_block.irq_block.cause_reg,result.cause);
        read_reg(registers.event_block.producer_reg,result.event_producer);
        read_reg(registers.control_block.recovery_block.fatal_reg,fatal_code);
        pop_event(result,available);
        if(!available) begin result.outcome=TASK_FATAL; result.code=fatal_code; end
    endtask
endclass
