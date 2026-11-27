class pdma_word_write_test extends pdma_base_test;
    `uvm_component_utils(pdma_word_write_test)
    function new(string name,uvm_component parent); super.new(name,parent); expected_cases=2; endfunction
    virtual task run_scenario();
        for(int w=4;w<=8;w*=2) begin
            pdma_task_flow flow_=env.device.task_flow();
            pdma_pattern_task job=new;
            job.opcode=COMMANDHEADER_OPCODE_WRITE;
            job.width=w; job.length=w; job.payload_alignment=w;
            job.dest_context=$urandom; job.pattern={$urandom,$urandom};
            flow_.open(job); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
        end
    endtask
endclass

class pdma_pattern_fill_test extends pdma_base_test;
    `uvm_component_utils(pdma_pattern_fill_test)
    function new(string name,uvm_component parent); super.new(name,parent); expected_cases=12; endfunction
    virtual task run_scenario();
        for(int i=0;i<expected_cases;i++) begin
            pdma_task_flow flow_=env.device.task_flow(i%platform.channel_count);
            pdma_pattern_task job=new;
            int w=i%3==0 ? 1 : i%3==1 ? 4 : 8;
            if(!job.randomize() with {width==local::w; cross_page==1; dest_lane>0;})
                `uvm_fatal("FILL_CONFIG","Cannot generate a patterned Fill")
            if(i==0) job.length=0;
            flow_.open(job); flow_.submit(); flow_.expect_fence(); flow_.close(); cases++;
        end
    endtask
endclass

class pdma_fill_throughput_test extends pdma_throughput_test;
    `uvm_component_utils(pdma_fill_throughput_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function pdma_task transfer();
        pdma_pattern_task job=new;
        job.length=transfer_bytes;
        return job;
    endfunction
endclass

class pdma_fill_replay_test extends pdma_base_test;
    `uvm_component_utils(pdma_fill_replay_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        localparam int BURST=pdma_configuration::COPY_BURST_BYTES;
        pdma_task_flow flow_=env.device.task_flow();
        pdma_pattern_task job=new;
        int offset=(`PDMA_COPY_SLOTS+1)*BURST;
        job.length=offset+2*BURST; job.payload_alignment=pdma_configuration::AXI_BOUNDARY_BYTES;
        flow_.open(job); inject(1,1,job.dest_address+offset); flow_.submit(); flow_.expect_fault();
        if(platform.channel_count>1) begin
            pdma_task_flow peer=env.device.task_flow(1);
            pdma_pattern_task other=new;
            // The first burst faults. At least one later burst must acquire a
            // new row after resume and use this channel's restored operand.
            other.width=1; other.pattern=~job.pattern;
            other.length=(`PDMA_COPY_SLOTS+1)*BURST;
            other.payload_alignment=pdma_configuration::AXI_BOUNDARY_BYTES;
            peer.open(other);
            inject(1,1,other.dest_address);
            peer.submit(); peer.expect_fault();
            // Both channels now own suspended commands with different patterns.
            platform.fault_enabled=0;
            peer.channel.resume_task(); peer.expect_fence(); peer.close();
        end
        // Earlier head rows already retired. A replay of one would overwrite this canary.
        if(env.memory.read_byte(job.dest_address)!==job.expected[0])
            `uvm_fatal("FILL_PREFIX","First burst was not visible at the mid-command fault")
        job.expected[0]=~job.expected[0]; env.memory.write_byte(job.dest_address,job.expected[0]);
        platform.fault_enabled=0;
        flow_.channel.resume_task(); flow_.expect_fence(); flow_.close(); cases++;
    endtask
endclass

class pdma_fill_error_test extends pdma_base_test;
    `uvm_component_utils(pdma_fill_error_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_pattern_task job=new;
        job.length=2*pdma_configuration::COPY_BURST_BYTES; job.payload_alignment=pdma_configuration::AXI_BOUNDARY_BYTES;
        flow_.open(job); inject(1,0,job.dest_address); flow_.submit(); flow_.expect_error(EVENT_CODE_WRITEERROR);
        platform.fault_enabled=0; flow_.channel.reset(); flow_.close(); cases++;
    endtask
endclass

function automatic Command_t timestamp_record(longint unsigned id,addr,bit [31:0] selector);
    CommandHeader_t header='0;
    TimestampCommand_t cmd='0;
    CommandHeader_set_opcode(header,COMMANDHEADER_OPCODE_TIMESTAMP);
    TimestampCommand_set_header(cmd,header); TimestampCommand_set_id(cmd,id);
    TimestampCommand_set_address(cmd,addr); TimestampCommand_set_context(cmd,selector);
    return Command_t'(cmd);
endfunction

class pdma_timestamp_fill_test extends pdma_base_test;
    `uvm_component_utils(pdma_timestamp_fill_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual task run_scenario();
        pdma_task_flow flow_=env.device.task_flow();
        pdma_pattern_task job=new;
        pdma_buffer markers[2];
        logic [63:0] first,last,lower,resumed;
        Command_t commands[$];
        job.length=1027; job.dest_context=$urandom;
        flow_.open(job);
        foreach(markers[i]) begin
            markers[i]=new; markers[i].allocate(flow_.allocator,8,8);
            env.memory.permit(markers[i].allocation.get_s_addr(),markers[i].address,8,job.dest_context,1,job.channel_id);
            for(int b=0;b<8;b++) env.memory.write_byte(markers[i].address+b,'1);
        end
        commands.push_back(timestamp_record(flow_.channel.next_id++,markers[0].address,job.dest_context));
        commands.push_back(job.command_record());
        commands.push_back(timestamp_record(flow_.channel.next_id++,markers[1].address,job.dest_context));
        commands.push_back(job.fence_record());
        lower=env.control.timer_value;
        inject(1,1,job.dest_address);
        flow_.submit_records(commands); flow_.expect_fault();
        first=env.memory.read_long(markers[0].address);
        if($isunknown(first) || first<lower || first>env.control.timer_value ||
            env.memory.read_long(markers[1].address)!==64'hffff_ffff_ffff_ffff)
            `uvm_fatal("TIMESTAMP_ORDER","Parked Fill must be preceded by exactly the first Timestamp")
        resumed=env.control.timer_value;
        platform.fault_enabled=0; flow_.channel.resume_task();
        flow_.expect_fence();
        last=env.memory.read_long(markers[1].address);
        if(env.memory.read_long(markers[0].address)!==first || $isunknown(last) || last<resumed || first>=last || last>env.control.timer_value)
            `uvm_fatal("TIMESTAMP_ORDER",$sformatf("timer range %0d..%0d, samples %0d..%0d",lower,env.control.timer_value,first,last))
        foreach(markers[i]) markers[i].deallocate(flow_.allocator,env.memory);
        flow_.close(); cases++;
    endtask
endclass
