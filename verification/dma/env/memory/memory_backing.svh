class pdma_memory extends jvs_memory_model;
    localparam int unsigned BEAT_BYTES=pdma_configuration::MEMORY_BEAT_BYTES;
    svt_mem backing;
    typedef struct {
        bit[63:0] owner, address, size;
        bit[31:0] selector;
        int unsigned channel;
        bit writing;
    } permission;
    local permission regions[$];
    function new(string name="memory"); super.new(name); endfunction
    function void permit(bit[63:0] owner,address,size,bit[31:0] selector,bit writing,int unsigned channel=0);
        permission region='{owner,address,size,selector,channel,writing};
        if(size!=0) regions.push_back(region);
    endfunction
    function void release_regions(bit[63:0] owner);
        for(int i=regions.size()-1;i>=0;i--) if(regions[i].owner==owner) regions.delete(i);
    endfunction
    function void assert_access(bit[63:0] address,size,bit[31:0] selector,bit writing,int unsigned channel=0);
        foreach(regions[i]) if(regions[i].channel==channel && regions[i].selector==selector && regions[i].writing==writing &&
            address>=regions[i].address && size<=regions[i].size &&
            address-regions[i].address<=regions[i].size-size) return;
        `uvm_fatal("DMA_MEMORY_PERMISSION",$sformatf("Unauthorized %s addr=%h size=%0d context=%h channel=%0d",
            writing ? "write" : "read",address,size,selector,channel))
    endfunction
    virtual function void write_byte(bit[63:0] addr, MEM_BYTE data);
        logic [`SVT_MEM_MAX_DATA_WIDTH-1:0] word='0;
        logic [`SVT_MEM_MAX_DATA_WIDTH/8-1:0] mask='0;
        word[(addr%BEAT_BYTES)*8+:8]=data; mask[addr%BEAT_BYTES]=1;
        if(!backing.write(addr-addr%BEAT_BYTES,word,mask))
            `uvm_fatal("MEMORY_WRITE","VIP backing rejected a software write")
    endfunction
    virtual function MEM_BYTE read_byte(bit[63:0] addr);
        logic [`SVT_MEM_MAX_DATA_WIDTH-1:0] word=backing.read(addr-addr%BEAT_BYTES);
        return word[(addr%BEAT_BYTES)*8+:8];
    endfunction
endclass
