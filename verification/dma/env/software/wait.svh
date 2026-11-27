class pdma_wait_task extends pdma_task;
    int unsigned width=8;
    bit [31:0] comparison=WAITCOMMAND_COMPARISON_EQUAL;
    bit [63:0] target_value;
    function new(string name="wait_task");
        super.new(name);
        opcode=COMMANDHEADER_OPCODE_WAIT;
        // Wait has a read-only dependency and no destination payload.
        length=0;
    endfunction
    function void store(pdma_memory memory,bit [63:0] value);
        for(int i=0;i<width;i++) memory.write_byte(source_address+i,value[i*8+:8]);
    endfunction
    virtual task allocate(jvs_memory_allocator allocator,pdma_memory memory);
        allocate_destination(allocator,memory);
        source_buffer=new;
        source_buffer.allocate(allocator,width,width);
        source_address=source_buffer.address;
        memory.permit(source_buffer.allocation.get_s_addr(),source_address,width,source_context,0,channel_id);
        store(memory,0);
    endtask
    virtual function Command_t command_record();
        CommandHeader_t header='0;
        WordTarget_t target='0;
        WaitCommand_t record_value='0;
        CommandHeader_set_opcode(header,opcode);
        WordTarget_set_address(target,source_address);
        WordTarget_set_context(target,source_context);
        WordTarget_set_width(target,width==4 ? WORDTARGET_WIDTH_BITS32 : WORDTARGET_WIDTH_BITS64);
        WaitCommand_set_header(record_value,header);
        WaitCommand_set_id(record_value,id);
        WaitCommand_set_target(record_value,target);
        WaitCommand_set_value(record_value,target_value);
        WaitCommand_set_comparison(record_value,comparison);
        return Command_t'(record_value);
    endfunction
endclass
