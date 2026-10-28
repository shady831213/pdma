use hw_model_schema::record_schema;

record_schema! {
    #[short_description = "Shared command discriminator"]
    #[description = "One byte at the start of every complete command record. The protocol selects the complete union member using this opcode."]
    pub record CommandHeader { alignment: 1, endian: Little, fields: {
        #[short_description = "Command opcode"]
        #[description = "Assigned opcodes 0 through 8; all other values are invalid."]
        opcode: enum(u8) { variants: { Copy = 0, Write = 1, Fill = 2, Wait = 3, Atomic = 4, Semaphore = 5, Fence = 6, Trap = 7, Timestamp = 8 } },
    } }
}
pub use CommandHeaderOpcode as Opcode;
