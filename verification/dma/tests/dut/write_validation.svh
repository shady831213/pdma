class pdma_word_geometry_test extends pdma_base_test;
    `uvm_component_utils(pdma_word_geometry_test)
    typedef enum {Unaligned32, HalfAligned64, EndWrap64} geometry_error;
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        geometry_error reason;
        super.build_phase(phase); expected_cases=platform.channel_count*reason.num();
    endfunction
    virtual task run_scenario();
        foreach(env.device.channels[i]) begin
            geometry_error reason=reason.first();
            repeat(reason.num()) begin
                pdma_task_flow flow_=env.device.task_flow(i);
                pdma_pattern_task job=new;
                WriteCommand_t cmd;
                WordWrite_t word_value;
                WordTarget_t target;
                longint unsigned address;
                Command_t records[$];
                job.opcode=COMMANDHEADER_OPCODE_WRITE;
                job.width=reason==Unaligned32 ? 4 : 8;
                job.length=2*job.width; job.payload_alignment=job.width;
                job.expect_payload_unchanged=1;
                flow_.open(job);
                case(reason)
                    Unaligned32: address=job.dest_address+1;
                    HalfAligned64: address=job.dest_address+4;
                    EndWrap64: address=-longint'(job.width);
                endcase
                cmd=WriteCommand_t'(job.command_record());
                word_value=WriteCommand_get_word(cmd); target=WordWrite_get_target(word_value);
                WordTarget_set_address(target,address); WordWrite_set_target(word_value,target);
                WriteCommand_set_word(cmd,word_value);
                records.push_back(Command_t'(cmd)); records.push_back(job.fence_record());
                flow_.submit_records(records); flow_.expect_error(EVENT_CODE_INVALIDOPERATION);
                flow_.channel.reset(); flow_.close(); cases++;
                reason=reason.next();
            end
        end
    endtask
endclass

class pdma_fill_range_test extends pdma_base_test;
    `uvm_component_utils(pdma_fill_range_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=platform.channel_count;
    endfunction
    virtual task run_scenario();
        foreach(env.device.channels[i]) begin
            pdma_task_flow flow_=env.device.task_flow(i);
            pdma_pattern_task job=new;
            FillCommand_t cmd;
            Command_t records[$];
            job.length=2*job.width; job.expect_payload_unchanged=1;
            flow_.open(job);
            cmd=FillCommand_t'(job.command_record());
            FillCommand_set_address(cmd,-longint'(job.width));
            records.push_back(Command_t'(cmd)); records.push_back(job.fence_record());
            flow_.submit_records(records); flow_.expect_error(EVENT_CODE_INVALIDLENGTH);
            flow_.channel.reset(); flow_.close(); cases++;
        end
    endtask
endclass

class pdma_timestamp_unavailable_test extends pdma_base_test;
    `uvm_component_utils(pdma_timestamp_unavailable_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        super.build_phase(phase); expected_cases=platform.channel_count;
    endfunction
    virtual task run_scenario();
        env.control.timer_available=0;
        foreach(env.device.channels[i]) begin
            pdma_task_flow flow_=env.device.task_flow(i);
            pdma_task job=settings(8);
            Command_t records[$];
            job.opcode=COMMANDHEADER_OPCODE_TIMESTAMP;
            job.payload_alignment=8; job.expect_payload_unchanged=1;
            flow_.open(job);
            records.push_back(timestamp_record(job.id,job.dest_address,job.dest_context));
            records.push_back(job.fence_record());
            flow_.submit_records(records); flow_.expect_error(EVENT_CODE_UNSUPPORTED);
            flow_.channel.reset(); flow_.close(); cases++;
        end
        env.control.timer_available=1;
    endtask
endclass
