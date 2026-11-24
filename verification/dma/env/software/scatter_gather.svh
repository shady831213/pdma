// Put the logical first fragment after the second in memory. A simple linear
// transfer cannot pass the byte oracle. Each table also contains an empty entry,
// an unused prefix and an unused tail; the VIP permits only selected entries.
class pdma_sg_task extends pdma_task;
    bit source_sg=1, dest_sg=1, short_destination;
    localparam int START=1, EMPTY=2, SECOND=3, COUNT=5;
    localparam int BEAT=pdma_configuration::MEMORY_BEAT_BYTES;
    rand bit[31:0] list_context;
    rand int unsigned source_first, dest_first;
    constraint fragments {
        source_first>0; source_first<length;
        dest_first>0; dest_first<length;
        solve length before source_first, dest_first;
    }
    pdma_buffer tables[2];
    function new(string name="sg_task");
        super.new(name); length=3*BEAT; payload_alignment=BEAT;
        source_first=BEAT; dest_first=2*BEAT;
        source_context=11; dest_context=22; list_context=33;
    endfunction
    function int unsigned first_length(bit dst);
        return dst ? dest_first : source_first;
    endfunction
    function longint unsigned address_at(bit dst,int unsigned index);
        longint unsigned base=dst ? dest_address : source_address;
        int unsigned first=first_length(dst);
        if(!(dst ? dest_sg : source_sg)) return base+index;
        return index<first ? base+(length-first)+index : base+(index-first);
    endfunction
    virtual function longint unsigned destination_byte(int unsigned index);
        return address_at(1,index);
    endfunction
    function void put_segment(pdma_memory memory,int side,int entry,longint unsigned address,size);
        Segment_t segment='0;
        Segment_set_address(segment,address); Segment_set_length(segment,size);
        for(int byte_no=0;byte_no<SEGMENT_SIZE;byte_no++)
            memory.write_byte(tables[side].address+entry*SEGMENT_SIZE+byte_no,segment[8*byte_no+:8]);
    endfunction
    virtual task allocate(jvs_memory_allocator allocator,pdma_memory memory);
        super.allocate(allocator,memory);
        foreach(tables[side]) if(side ? dest_sg : source_sg) begin
            int unsigned first=first_length(side);
            tables[side]=new; tables[side].allocate(allocator,COUNT*SEGMENT_SIZE,SEGMENT_ALIGNMENT);
            memory.permit(tables[side].allocation.get_s_addr(),tables[side].address+START*SEGMENT_SIZE,
                (SECOND-START+1)*SEGMENT_SIZE,list_context,0,channel_id);
            put_segment(memory,side,START,address_at(side,0)-1,first+1);
            put_segment(memory,side,EMPTY,'1,0);
            put_segment(memory,side,SECOND,address_at(side,first),length-first);
        end
        foreach(expected[i]) begin
            memory.write_byte(address_at(0,i),expected[i]);
            memory.write_byte(destination_byte(i),initial_data[i]);
        end
    endtask
    virtual task deallocate(jvs_memory_allocator allocator,pdma_memory memory);
        foreach(tables[side]) if(tables[side]!=null) tables[side].deallocate(allocator,memory);
        super.deallocate(allocator,memory);
    endtask
    virtual function Command_t command_record();
        CopyCommand_t command=CopyCommand_t'(super.command_record());
        CopyCommand_set_options(command,(source_sg << COPYCOMMAND_OPTIONS_SOURCE_MODE_LSB) |
            (dest_sg << COPYCOMMAND_OPTIONS_DESTINATION_MODE_LSB));
        CopyCommand_set_sg_context(command,list_context);
        foreach(tables[side]) if(tables[side]!=null) begin
            ScatterGatherEndpoint_t endpoint='0;
            ScatterGatherEndpoint_set_list_address(endpoint,tables[side].address);
            ScatterGatherEndpoint_set_count(endpoint,(side && short_destination) ? SECOND : COUNT);
            ScatterGatherEndpoint_set_index(endpoint,START);
            ScatterGatherEndpoint_set_offset(endpoint,1);
            if(side) CopyCommand_set_destination(command,endpoint);
            else CopyCommand_set_source(command,endpoint);
        end
        return Command_t'(command);
    endfunction
endclass
