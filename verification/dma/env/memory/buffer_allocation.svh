class pdma_buffer extends uvm_object;
    localparam int unsigned PAGE_BYTES=pdma_configuration::AXI_BOUNDARY_BYTES;
    localparam int unsigned BEAT_BYTES=pdma_configuration::MEMORY_BEAT_BYTES;
    jvs_memory_acc_block allocation;
    longint unsigned address;
    function new(string name="buffer"); super.new(name); endfunction
    task allocate(jvs_memory_allocator allocator,int unsigned size,int unsigned alignment=1,
        bit cross_page=0,int unsigned lane=0,bit guards=0);
        int unsigned prefix=guards ? 1 : 0;
        int unsigned extent=size+prefix+(guards ? 1 : 0);
        if(extent==0) extent=1;
        if(cross_page) begin
            for(longint unsigned page=(allocator.range.get_s_addr()+PAGE_BYTES-1)/PAGE_BYTES*PAGE_BYTES;
                page<allocator.range.get_e_addr();page+=PAGE_BYTES) begin
                longint unsigned candidate=page-BEAT_BYTES+lane-prefix;
                if(candidate<allocator.range.get_s_addr() || candidate+extent-1>allocator.range.get_e_addr()) continue;
                allocator.malloc(allocation,extent,ALIGN_BYTE,{1'b0,candidate});
                if(allocation!=null) break;
            end
            if(allocation!=null) address=allocation.get_s_addr()+prefix;
        end else begin
            allocator.malloc(allocation,extent+alignment-1+lane,ALIGN_BYTE);
            if(allocation!=null) address=((allocation.get_s_addr()+prefix+alignment-1)/alignment)*alignment+lane;
        end
        if(allocation==null) `uvm_fatal("BUFFER_ALLOCATION","Cannot allocate independent software buffer")
    endtask
    task deallocate(jvs_memory_allocator allocator,pdma_memory memory);
        memory.release_regions(allocation.get_s_addr()); allocator.free(allocation);
    endtask
endclass
