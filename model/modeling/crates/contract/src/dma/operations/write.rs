use super::super::command::CommandHeader;
use super::WordWrite;
use hw_model_schema::record_schema;

record_schema! {
    #[short_description = "Complete Write command"]
    #[description = "Complete Write command including shared header and software identifier."]
    pub record WriteCommand { alignment: 8, endian: Little, fields: {
            #[short_description = "Shared command header"]
            #[description = "Opcode selecting this complete command."]
            header: CommandHeader,
            #[short_description = "Software identifier"]
            #[description = "Software identifier retained in error events."]
            id: u64,
        #[short_description = "Word store"] #[description = "Target and value of the word store."]
        word: WordWrite,
    } }
}
record_schema! {
    #[short_description = "Complete Fence command"]
    #[description = "Complete Fence command including shared header and software identifier."]
    pub record FenceCommand { alignment: 8, endian: Little, fields: {
            #[short_description = "Shared command header"]
            #[description = "Opcode selecting this complete command."]
            header: CommandHeader,
            #[short_description = "Software identifier"]
            #[description = "Software identifier retained in error events."]
            id: u64,
        #[short_description = "Word store"] #[description = "Target and value of the word store."]
        word: WordWrite,
    } }
}
record_schema! {
    #[short_description = "Complete Trap command"]
    #[description = "Complete Trap command including shared header and software identifier."]
    pub record TrapCommand { alignment: 8, endian: Little, fields: {
            #[short_description = "Shared command header"]
            #[description = "Opcode selecting this complete command."]
            header: CommandHeader,
            #[short_description = "Software identifier"]
            #[description = "Software identifier retained in error events."]
            id: u64,
    } }
}
