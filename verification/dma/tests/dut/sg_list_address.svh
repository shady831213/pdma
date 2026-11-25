// Invalid metadata addresses must stop the command before any list or payload read.
class pdma_invalid_list_task extends pdma_sg_task;
    typedef enum {Exhausted, IndexWrap, AddressWrap, EntryEndWrap, Unaligned} list_error;
    list_error reason;

    virtual function Command_t command_record();
        CopyCommand_t command=CopyCommand_t'(super.command_record());
        ScatterGatherEndpoint_t endpoint=CopyCommand_get_source(command);
        longint unsigned address=tables[0].address, index=0, count='1;
        case(reason)
            // Exhaustion takes precedence even if computing the address would overflow.
            Exhausted: begin index='1; count=index; end
            IndexWrap: index=(64'hffff_ffff_ffff_ffff / SEGMENT_SIZE)+1;
            AddressWrap: begin address=-longint'(SEGMENT_SIZE); index=1; end
            EntryEndWrap: address=-longint'(SEGMENT_SIZE);
            Unaligned: address++;
        endcase
        ScatterGatherEndpoint_set_list_address(endpoint,address);
        ScatterGatherEndpoint_set_index(endpoint,index);
        ScatterGatherEndpoint_set_count(endpoint,count);
        CopyCommand_set_source(command,endpoint);
        return Command_t'(command);
    endfunction
endclass

class pdma_sg_list_address_test extends pdma_base_test;
    `uvm_component_utils(pdma_sg_list_address_test)
    function new(string name,uvm_component parent); super.new(name,parent); endfunction
    virtual function void build_phase(uvm_phase phase);
        pdma_invalid_list_task::list_error reason;
        super.build_phase(phase); expected_cases=platform.channel_count*reason.num();
    endfunction
    virtual task run_scenario();
        foreach(env.device.channels[ch]) begin
            pdma_invalid_list_task::list_error reason;
            pdma_task_flow flow_=env.device.task_flow(ch);
            reason=reason.first();
            repeat(reason.num()) begin
                pdma_invalid_list_task job=new;
                job.dest_sg=0; job.expect_payload_unchanged=1; job.reason=reason;
                flow_.open(job);
                env.memory.release_regions(job.tables[0].allocation.get_s_addr());
                env.memory.release_regions(job.source_buffer.allocation.get_s_addr());
                flow_.submit();
                case(reason)
                    pdma_invalid_list_task::Exhausted: flow_.expect_error(EVENT_CODE_LENGTHMISMATCH);
                    pdma_invalid_list_task::Unaligned: flow_.expect_error(EVENT_CODE_LISTREADERROR);
                    default: flow_.expect_error(EVENT_CODE_INVALIDLENGTH);
                endcase
                flow_.channel.reset(); flow_.close(); cases++;
                reason=reason.next();
            end
        end
    endtask
endclass
