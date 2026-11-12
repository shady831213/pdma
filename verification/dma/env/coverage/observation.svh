class pdma_observation extends uvm_object;
    int unsigned channel_id, length, source_lane, dest_lane;
    longint unsigned task_id;
    pdma_outcome outcome;
    realtime latency;
    bit cross_page, event_cross_page, fence_wide;
    function new(string name="observation"); super.new(name); endfunction
endclass
