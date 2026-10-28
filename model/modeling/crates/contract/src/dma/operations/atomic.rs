use super::super::command::CommandHeader;
use super::WordTarget;
use hw_model_schema::record_schema;

record_schema! {
    #[short_description = "One atomic effect with no returned-result storage"]
    #[description = "One atomic effect with no returned-result storage."]
    pub record AtomicCommand {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Shared command header"]
            #[description = "Opcode selecting this complete command."]
            header: CommandHeader,
            #[short_description = "Software identifier"]
            #[description = "Software identifier retained in error events."]
            id: u64,
            #[short_description = "RMW target; requires discovered width capability"]
            #[description = "RMW target; requires discovered width capability."]
            target: WordTarget,
            #[short_description = "Memory ordering for the atomic operation"]
            #[description = "Memory ordering for the atomic operation."]
            order: enum(u32) { variants: { Relaxed = 0, Acquire = 1, Release = 2, AcquireRelease = 3 } },
            #[short_description = "Selected atomic read-modify-write operation"]
            #[description = "Selected atomic read-modify-write operation."]
            operation: enum(u32) { variants: { Add = 0, Subtract = 1, And = 2, Or = 3, Xor = 4, Exchange = 5, CompareExchange = 6 } },
            #[short_description = "Width-truncated bit patterns"]
            #[description = "Width-truncated bit patterns. Arithmetic wraps. CAS mismatch succeeds unchanged; no software old-value destination."]
            operands: tagged_union { tag: self.operation, cases: { Add { value: u64 }, Subtract { value: u64 }, And { value: u64 }, Or { value: u64 }, Xor { value: u64 }, Exchange { value: u64 }, CompareExchange { compare: u64, value: u64 } } },
        },
    }
}
