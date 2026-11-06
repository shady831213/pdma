class pdma_channel extends uvm_object;
    int unsigned channel_id, vector, producer;
    longint unsigned next_id=1;
    svt_axi_master_sequencer mmio;
    pdma_memory memory;
    virtual pdma_control_if control;
    bit[31:0] command_base, event_base, config_base, recovery_base, irq_base, reset_address, global_base;
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
    function void bind_registers(int count,int index,int irq_vector);
        longint unsigned base=pdma_rtl_DmaRegistersLayout::channels_offset(count)+
            index*pdma_rtl_DmaRegistersLayout::channels_stride(count);
        longint unsigned registers=base+pdma_rtl_ChannelBlockLayout::control_offset();
        longint unsigned admin=pdma_rtl_DmaRegistersLayout::admin_offset(count);
        channel_id=index; vector=irq_vector;
        command_base=base+pdma_rtl_ChannelBlockLayout::command_offset();
        event_base=base+pdma_rtl_ChannelBlockLayout::event_offset();
        config_base=registers+pdma_rtl_ChannelRegistersLayout::config_offset();
        recovery_base=registers+pdma_rtl_ChannelRegistersLayout::recovery_offset();
        irq_base=registers+pdma_rtl_ChannelRegistersLayout::irq_offset();
        reset_address=admin+pdma_rtl_DmaAdminLayout::channels_offset(count)+index*pdma_rtl_DmaAdminLayout::channels_stride(count);
        global_base=admin+pdma_rtl_DmaAdminLayout::control_offset(count);
    endfunction
    task write_reg(bit[31:0] address,bit[31:0] value);
        pdma_mmio seq=new;
        seq.write_access=1; seq.address=address; seq.value=value; seq.start(mmio);
    endtask
    task read_reg(bit[31:0] address,output bit[31:0] value);
        pdma_mmio seq=new;
        seq.address=address; seq.start(mmio); value=seq.value;
    endtask
    task await_reg(bit[31:0] address,bit[31:0] mask,bit[31:0] expected);
        bit[31:0] value;
        for(int n=0;n<4000;n++) begin
            read_reg(address,value);
            if((value & mask)==expected) return;
        end
        `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d register=%h expected=%h observed=%h",channel_id,address,expected,value))
    endtask
    task setup();
        allocator.malloc(command_allocation,cfg.command_capacity*COMMAND_SIZE,e_alignment'($clog2(COMMAND_ALIGNMENT)));
        allocator.malloc(event_allocation,cfg.event_capacity*EVENT_SIZE,e_alignment'($clog2(EVENT_ALIGNMENT)));
        if(command_allocation==null || event_allocation==null) `uvm_fatal("RING_ALLOCATION","Channel ring allocation failed")
        command_address=command_allocation.get_s_addr(); event_address=event_allocation.get_s_addr();
        memory.permit(command_address,command_address,cfg.command_capacity*COMMAND_SIZE,cfg.ring_context,0);
        memory.permit(event_address,event_address,cfg.event_capacity*EVENT_SIZE,cfg.ring_context,1);
        configure();
    endtask
    task configure();
        producer=0; event_consumer=0;
        write_reg(command_base+pdma_rtl_CommandRegistersLayout::BASE_OFFSET,command_address);
        write_reg(command_base+pdma_rtl_CommandRegistersLayout::BASE_OFFSET+4,command_address>>32);
        write_reg(command_base+pdma_rtl_CommandRegistersLayout::CAPACITY_OFFSET,cfg.command_capacity);
        write_reg(event_base+pdma_rtl_EventRegistersLayout::BASE_OFFSET,event_address);
        write_reg(event_base+pdma_rtl_EventRegistersLayout::BASE_OFFSET+4,event_address>>32);
        write_reg(event_base+pdma_rtl_EventRegistersLayout::CAPACITY_OFFSET,cfg.event_capacity);
        write_reg(config_base+pdma_rtl_ChannelConfigurationLayout::RING_CONTEXT_OFFSET,cfg.ring_context);
        write_reg(global_base+pdma_rtl_DmaControlLayout::IRQ_ENABLE_OFFSET,1);
        write_reg(irq_base+pdma_rtl_ChannelInterruptsLayout::ENABLE_OFFSET,1);
        write_reg(config_base+pdma_rtl_ChannelConfigurationLayout::ENABLE_OFFSET,1);
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
        read_reg(command_base+pdma_rtl_CommandRegistersLayout::CONSUMER_OFFSET,consumer);
        if(producer-consumer+(publish_fence ? 2 : 1)>cfg.command_capacity)
            `uvm_fatal("COMMAND_RING_FULL","No command ring capacity for task")
        publish_record(job.copy_record(),producer);
        if(publish_fence) publish_record(job.fence_record(),producer+1);
        job.submitted_at=$realtime;
        producer+=publish_fence ? 2 : 1;
        write_reg(command_base+pdma_rtl_CommandRegistersLayout::PRODUCER_OFFSET,producer);
        producer_lock.put();
    endtask
    task await_consumed();
        await_reg(command_base+pdma_rtl_CommandRegistersLayout::CONSUMER_OFFSET,'1,producer);
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
        read_reg(event_base+pdma_rtl_EventRegistersLayout::PRODUCER_OFFSET,published);
        available=published!=event_consumer;
        if(available) begin
            address=event_address+(event_consumer%cfg.event_capacity)*EVENT_SIZE;
            for(int i=0;i<EVENT_SIZE;i++) event_value[i*8+:8]=memory.read_byte(address+i);
            result.outcome=TASK_ERROR; result.task_id=Event_get_id(event_value);
            result.code=Event_get_code(event_value); info=Event_get_info(event_value);
            result.prefix=ErrorInfo_get_bytes_completed(info); result.event_producer=published;
            event_consumer++;
            write_reg(event_base+pdma_rtl_EventRegistersLayout::CONSUMER_OFFSET,event_consumer);
        end
        event_lock.put();
    endtask
    task request_reset(bit global_scope=0);
        write_reg(global_scope ? global_base+pdma_rtl_DmaControlLayout::RESET_OFFSET : reset_address,1);
    endtask
    task finish_reset(bit global_scope=0);
        await_reg(global_scope ? global_base+pdma_rtl_DmaControlLayout::RESET_OFFSET : reset_address,1,0);
        producer=0; event_consumer=0; configured=0;
    endtask
    task resume_task(); write_reg(recovery_base+pdma_rtl_ChannelRecoveryLayout::RESUME_OFFSET,1); endtask
    task observe(pdma_task job,output pdma_result result);
        bit[31:0] fatal_code, faulted, producer_value, cause;
        longint unsigned value;
        bit available;
        result=null;
        read_reg(recovery_base+pdma_rtl_ChannelRecoveryLayout::FATAL_OFFSET,fatal_code);
        read_reg(recovery_base+pdma_rtl_ChannelRecoveryLayout::FAULT_STATUS_OFFSET,faulted);
        read_reg(event_base+pdma_rtl_EventRegistersLayout::PRODUCER_OFFSET,producer_value);
        read_reg(irq_base+pdma_rtl_ChannelInterruptsLayout::CAUSE_OFFSET,cause);
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
        for(int n=0;n<4000;n++) begin
            observe(job,result);
            if(result!=null) return;
        end
        `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d task=%0d has no completion",channel_id,job.id))
    endtask
    task wait_interrupt(output pdma_result result);
        fork
            interrupts.get(result);
            begin
                repeat(4000) @(posedge control.core_clk);
                `uvm_fatal("TASK_TIMEOUT",$sformatf("channel=%0d IRQ did not arrive",channel_id))
            end
        join_any
        disable fork;
    endtask
    task interrupt_snapshot(output pdma_result result);
        bit[31:0] fatal_code;
        bit available;
        result=new; result.channel_id=channel_id;
        read_reg(irq_base+pdma_rtl_ChannelInterruptsLayout::CAUSE_OFFSET,result.cause);
        read_reg(event_base+pdma_rtl_EventRegistersLayout::PRODUCER_OFFSET,result.event_producer);
        read_reg(recovery_base+pdma_rtl_ChannelRecoveryLayout::FATAL_OFFSET,fatal_code);
        pop_event(result,available);
        if(!available) begin result.outcome=TASK_FATAL; result.code=fatal_code; end
    endtask
endclass
