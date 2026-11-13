class pdma_task extends uvm_object;
    int unsigned channel_id;
    longint unsigned id;
    rand int unsigned length, source_lane, dest_lane, source_offset, dest_offset;
    rand bit cross_page, fence_wide;
    rand bit[31:0] fence_context, source_context, dest_context;
    int unsigned max_length=257;
    constraint geometry {
        length inside {[1:max_length]}; source_lane inside {[0:7]}; dest_lane inside {[0:7]};
        source_offset inside {[0:31]}; dest_offset inside {[0:31]};
    }
    pdma_buffer source_buffer, dest_buffer, fence_buffer;
    longint unsigned source_address, dest_address, fence_address, source_base;
    int unsigned payload_alignment=1;
    logic [7:0] opcode=COMMANDHEADER_OPCODE_COPY;
    byte unsigned expected[], initial_data[];
    bit terminal_checked;
    bit expect_payload_unchanged;
    longint unsigned expected_error_prefix;
    bit allow_zero_error_prefix=0;
    realtime submitted_at;
    function new(string name="task"); super.new(name); fence_wide=1; endfunction
    function longint unsigned fence_value(); return id; endfunction
    function int unsigned fence_bytes(); return fence_wide ? $bits(longint)/8 : $bits(int)/8; endfunction
    task allocate(jvs_memory_allocator allocator,pdma_memory memory);
        source_buffer=new; dest_buffer=new; fence_buffer=new;
        source_buffer.allocate(allocator,length,payload_alignment,cross_page,source_lane);
        dest_buffer.allocate(allocator,length,payload_alignment,cross_page,dest_lane,1);
        fence_buffer.allocate(allocator,fence_bytes(),fence_bytes());
        source_address=source_buffer.address; dest_address=dest_buffer.address; fence_address=fence_buffer.address;
        source_base=source_address-source_offset;
        expected=new[length]; initial_data=new[length];
        memory.permit(source_buffer.allocation.get_s_addr(),source_address,length,source_context,0,channel_id);
        memory.permit(dest_buffer.allocation.get_s_addr(),dest_address,length,dest_context,1,channel_id);
        memory.permit(fence_buffer.allocation.get_s_addr(),fence_address,fence_bytes(),fence_context,1,channel_id);
        foreach(expected[i]) begin
            expected[i]=$urandom; initial_data[i]=~expected[i];
            memory.write_byte(source_address+i,expected[i]); memory.write_byte(dest_address+i,initial_data[i]);
        end
        memory.write_byte(dest_address-1,8'ha6); memory.write_byte(dest_address+length,8'h59);
        for(int i=0;i<fence_bytes();i++) memory.write_byte(fence_address+i,0);
    endtask
    task deallocate(jvs_memory_allocator allocator,pdma_memory memory);
        source_buffer.deallocate(allocator,memory); dest_buffer.deallocate(allocator,memory); fence_buffer.deallocate(allocator,memory);
    endtask
    function Command_t copy_record();
        CopyCommand_t record_value='0;
        CommandHeader_t header='0;
        LinearEndpoint_t source='0, destination='0;
        CommandHeader_set_opcode(header,opcode);
        LinearEndpoint_set_address(source,source_base); LinearEndpoint_set_offset(source,source_offset);
        LinearEndpoint_set_address(destination,dest_address-dest_offset); LinearEndpoint_set_offset(destination,dest_offset);
        CopyCommand_set_header(record_value,header); CopyCommand_set_id(record_value,id);
        CopyCommand_set_copy_length(record_value,length);
        CopyCommand_set_source_context(record_value,source_context); CopyCommand_set_destination_context(record_value,dest_context);
        CopyCommand_set_source(record_value,source); CopyCommand_set_destination(record_value,destination);
        return Command_t'(record_value);
    endfunction
    function Command_t fence_record();
        FenceCommand_t record_value='0;
        CommandHeader_t header='0;
        WordTarget_t target='0;
        WordWrite_t word_value='0;
        CommandHeader_set_opcode(header,COMMANDHEADER_OPCODE_FENCE);
        WordTarget_set_address(target,fence_address); WordTarget_set_context(target,fence_context);
        WordTarget_set_width(target,fence_wide ? WORDTARGET_WIDTH_BITS64 : WORDTARGET_WIDTH_BITS32);
        WordWrite_set_target(word_value,target); WordWrite_set_value(word_value,fence_value());
        FenceCommand_set_header(record_value,header); FenceCommand_set_id(record_value,id+1);
        FenceCommand_set_word(record_value,word_value);
        return Command_t'(record_value);
    endfunction
endclass
