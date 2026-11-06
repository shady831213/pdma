class pdma_memory extends jvs_memory_model;
    svt_mem backing;
    typedef struct {
        bit[63:0] owner, address, size;
        bit[31:0] selector;
        bit writing;
    } permission;
    local permission regions[$];
    function new(string name="memory"); super.new(name); endfunction
    function void permit(bit[63:0] owner,address,size,bit[31:0] selector,bit writing);
        permission region='{owner,address,size,selector,writing};
        if(size!=0) regions.push_back(region);
    endfunction
    function void release_regions(bit[63:0] owner);
        for(int i=regions.size()-1;i>=0;i--) if(regions[i].owner==owner) regions.delete(i);
    endfunction
    function void assert_access(bit[63:0] address,size,bit[31:0] selector,bit writing);
        foreach(regions[i]) if(regions[i].selector==selector && regions[i].writing==writing &&
            address>=regions[i].address && size<=regions[i].size &&
            address-regions[i].address<=regions[i].size-size) return;
        `uvm_fatal("DMA_MEMORY_PERMISSION",$sformatf("Unauthorized %s addr=%h size=%0d context=%h",
            writing ? "write" : "read",address,size,selector))
    endfunction
    virtual function void write_byte(bit[63:0] addr, MEM_BYTE data);
        logic [`SVT_MEM_MAX_DATA_WIDTH-1:0] word='0;
        logic [`SVT_MEM_MAX_DATA_WIDTH/8-1:0] mask='0;
        word[(addr%8)*8+:8]=data; mask[addr%8]=1;
        if(!backing.write(addr & ~64'd7,word,mask))
            `uvm_fatal("MEMORY_WRITE","VIP backing rejected a software write")
    endfunction
    virtual function MEM_BYTE read_byte(bit[63:0] addr);
        logic [`SVT_MEM_MAX_DATA_WIDTH-1:0] word=backing.read(addr & ~64'd7);
        return word[(addr%8)*8+:8];
    endfunction
endclass
