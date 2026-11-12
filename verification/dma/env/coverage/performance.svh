// Optional passive VIP callbacks. No per-cycle sampler, shadow responder or DUT probes.
// Register before submission; measure only this job's non-overlapping allocations.
typedef class pdma_performance;
class pdma_performance_phases extends svt_axi_port_monitor_callback;
    pdma_performance performance;
    function new(string name="performance_phases"); super.new(name); endfunction
    virtual function void read_address_phase_ended(svt_axi_port_monitor monitor_,svt_axi_transaction item);
        performance.read_address(item);
    endfunction
    virtual function void read_data_phase_ended(svt_axi_port_monitor monitor_,svt_axi_transaction item);
        performance.beat(0,item);
    endfunction
    virtual function void write_data_phase_ended(svt_axi_port_monitor monitor_,svt_axi_transaction item);
        performance.beat(1,item);
    endfunction
endclass

class pdma_performance extends uvm_subscriber#(svt_axi_transaction);
    pdma_task job;
    longint unsigned command_address;
    realtime period, first[2], last[2], descriptor_at, fence_at;
    int unsigned beats[2], bubbles[2], max_gap[2];
    int unsigned read_beats[longint unsigned], write_beats[longint unsigned];
    realtime read_ended[longint unsigned], minimum_handoff, maximum_handoff;
    int handoffs;
    bit enabled;
    pdma_performance_phases phases;
    function new(string name,uvm_component parent);
        super.new(name,parent); phases=new; phases.performance=this;
    endfunction
    function void attach(svt_axi_port_monitor monitor_);
        uvm_callbacks#(svt_axi_port_monitor,svt_axi_port_monitor_callback)::add(monitor_,phases);
        monitor_.item_observed_port.connect(analysis_export);
    endfunction
    function void begin_task(pdma_task task_,realtime bus_period,longint unsigned descriptor_address);
        if(enabled || bus_period<=0) `uvm_fatal("PERF_WINDOW","Invalid performance window")
        job=task_; period=bus_period; command_address=descriptor_address; descriptor_at=0; fence_at=0;
        foreach(beats[i]) begin beats[i]=0; bubbles[i]=0; max_gap[i]=0; first[i]=0; last[i]=0; end
        read_beats.delete(); write_beats.delete(); read_ended.delete();
        minimum_handoff=0; maximum_handoff=0; handoffs=0;
        enabled=1;
    endfunction
    local function int cycles(realtime interval_);
        int result_=int'(interval_/period);
        if(interval_<0 || $abs(interval_-result_*period)>0.001)
            `uvm_fatal("PERF_CLOCK",$sformatf("Off-cycle interval=%0.6f period=%0.6f",interval_,period))
        return result_;
    endfunction
    function void beat(bit writing,svt_axi_transaction item);
        longint unsigned start_;
        longint unsigned offset;
        int gap;
        if(!enabled) return;
        start_=writing ? job.dest_address : job.source_address;
        if(item.addr<start_ || item.addr>=start_+job.length) return;
        offset=item.addr-start_;
        if(!writing) begin
            if(!read_beats.exists(offset)) read_beats[offset]=0;
            read_beats[offset]++;
            if(read_beats[offset]==item.burst_length) read_ended[offset]=$realtime;
        end else begin
            if(!write_beats.exists(offset)) write_beats[offset]=0;
            if(write_beats[offset]==0 && read_ended.exists(offset)) begin
                realtime latency=$realtime-read_ended[offset];
                if(handoffs==0 || latency<minimum_handoff) minimum_handoff=latency;
                if(handoffs==0 || latency>maximum_handoff) maximum_handoff=latency;
                handoffs++;
            end
            write_beats[offset]++;
        end
        if(beats[writing]==0) first[writing]=$realtime;
        else begin
            gap=cycles($realtime-last[writing])-1;
            if(gap<0) `uvm_fatal("PERF_BEAT","Duplicate beat observation")
            bubbles[writing]+=gap;
            if(gap>max_gap[writing]) max_gap[writing]=gap;
            if(gap!=0 && bubbles[writing]<=8)
                `uvm_info("COPY_GAP",$sformatf("direction=%s before_beat=%0d gap_cycles=%0d logical_offset=%0d",
                    writing ? "write" : "read",beats[writing],gap,offset),UVM_NONE)
        end
        last[writing]=$realtime; beats[writing]++;
    endfunction
    function void read_address(svt_axi_transaction item);
        if(enabled && descriptor_at==0 && item.addr==command_address) descriptor_at=$realtime;
    endfunction
    virtual function void write(svt_axi_transaction item);
        if(enabled && item.xact_type==svt_axi_transaction::WRITE && item.addr==job.fence_address)
            fence_at=$realtime;
    endfunction
    function void finish(int unsigned write_latency_budget=0);
        int expected_beats=job.length/pdma_configuration::MEMORY_BEAT_BYTES;
        if(!enabled || descriptor_at==0 || fence_at==0 || expected_beats==0 ||
            beats[0]!=expected_beats || beats[1]!=expected_beats)
            `uvm_fatal("PERF_EMPTY",$sformatf("Incomplete observation read=%0d write=%0d expected=%0d descriptor=%0.3f fence=%0.3f",
                beats[0],beats[1],expected_beats,descriptor_at,fence_at))
        enabled=0;
        foreach(beats[i])
            `uvm_info("COPY_THROUGHPUT",$sformatf("bytes=%0d direction=%s beats=%0d span_cycles=%0d bubbles=%0d max_gap=%0d bytes_per_cycle=%0.4f",
                job.length,i ? "write" : "read",beats[i],cycles(last[i]-first[i])+1,bubbles[i],max_gap[i],
                real'(job.length)/(cycles(last[i]-first[i])+1)),UVM_NONE)
        `uvm_info("COPY_END_TO_END",$sformatf("bytes=%0d descriptor_to_fence_cycles=%0d submission_to_fence_ns=%0.3f",
            job.length,cycles(fence_at-descriptor_at),fence_at-job.submitted_at),UVM_NONE)
        if(handoffs!=0)
            `uvm_info("COPY_HANDOFF",$sformatf("bursts=%0d last_R_to_first_W_min=%0d max=%0d bus_cycles",
                handoffs,cycles(minimum_handoff),cycles(maximum_handoff)),UVM_NONE)
        `uvm_info("COPY_BUBBLE_BUDGET",$sformatf("read=0 write=%0d cycles over the entire payload",write_latency_budget),UVM_NONE)
        if(bubbles[0]!=0 || bubbles[1]>write_latency_budget)
            `uvm_fatal("PERF_PAYLOAD_BUBBLE","Payload gaps exceed the declared transport-latency budget")
    endfunction
endclass
