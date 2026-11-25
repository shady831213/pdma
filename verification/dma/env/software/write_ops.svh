// Write-only jobs grant the DUT no payload read permission.
class pdma_pattern_task extends pdma_task;
    rand bit [63:0] pattern;
    rand int unsigned width;
    constraint pattern_width {width inside {1,4,8};}
    function new(string name="pattern_task");
        super.new(name); opcode=COMMANDHEADER_OPCODE_FILL; width=8;
        pattern=64'hf103a65b924d87ce;
    endfunction
    virtual task allocate(jvs_memory_allocator allocator,pdma_memory memory);
        allocate_destination(allocator,memory);
        foreach(expected[i]) begin
            expected[i]=pattern[(i%width)*8+:8];
            initial_data[i]=~expected[i];
            memory.write_byte(dest_address+i,initial_data[i]);
        end
    endtask
    virtual function Command_t command_record();
        CommandHeader_t header='0;
        CommandHeader_set_opcode(header,opcode);
        if(opcode==COMMANDHEADER_OPCODE_WRITE) begin
            WriteCommand_t record_value='0;
            WordTarget_t target='0;
            WordWrite_t word_value='0;
            WordTarget_set_address(target,dest_address); WordTarget_set_context(target,dest_context);
            WordTarget_set_width(target,width==8 ? WORDTARGET_WIDTH_BITS64 : WORDTARGET_WIDTH_BITS32);
            WordWrite_set_target(word_value,target); WordWrite_set_value(word_value,pattern);
            WriteCommand_set_header(record_value,header); WriteCommand_set_id(record_value,id);
            WriteCommand_set_word(record_value,word_value); return Command_t'(record_value);
        end else begin
            FillCommand_t record_value='0;
            FillCommand_set_header(record_value,header); FillCommand_set_id(record_value,id);
            FillCommand_set_address(record_value,dest_address); FillCommand_set_context(record_value,dest_context);
            FillCommand_set_length(record_value,length); FillCommand_set_pattern(record_value,pattern);
            FillCommand_set_width(record_value,width==1 ? FILLCOMMAND_WIDTH_BYTE :
                width==4 ? FILLCOMMAND_WIDTH_BITS32 : FILLCOMMAND_WIDTH_BITS64);
            return Command_t'(record_value);
        end
    endfunction
endclass
