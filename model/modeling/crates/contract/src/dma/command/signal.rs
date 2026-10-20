use hardware_interface_schema::record_schema;

record_schema! {
    #[short_description = "Signal location"]
    #[description = "An aligned 64-bit signal in a selected address space. Software retains the signal allocation and binding until all accesses retire."]
    pub record SignalTarget {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Signal address"]
            #[description = "Eight-byte-aligned address of one atomic 64-bit signal."]
            address: u64,
            #[short_description = "Signal context"]
            #[description = "Address-space context selector for the signal access."]
            context: u32,
        },
    }
}

record_schema! {
    #[short_description = "Atomic signal command"]
    #[description = "One atomic access to a signed 64-bit signal. Values use two's-complement u64 wire storage. The response returns the observed old value; compare exchange updates only on equality."]
    pub record SignalCommand {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Signal target"]
            #[description = "Address and context selecting the signal allocation."]
            target: SignalTarget,
            #[short_description = "Atomic operation"]
            #[description = "Load, store, wrapping arithmetic, bitwise operations, exchange or compare exchange. Arithmetic operates on the 64-bit representation."]
            operation: enum(u32) { variants: {
                Load = 0, Store = 1, Add = 2, Subtract = 3, And = 4,
                Or = 5, Xor = 6, Exchange = 7, CompareExchange = 8,
            } },
            #[short_description = "Memory order"]
            #[description = "Load allows Relaxed or Acquire; Store allows Relaxed or Release; read-modify-write operations allow all four orders. Compare-exchange failure uses Acquire for acquire-bearing orders and Relaxed otherwise."]
            order: enum(u32) { variants: {
                Relaxed = 0, Acquire = 1, Release = 2, AcquireRelease = 3,
            } },
            #[short_description = "Operation operand"]
            #[description = "Two's-complement operand or replacement value; ignored by Load."]
            value: u64,
            #[short_description = "Expected value"]
            #[description = "Two's-complement equality operand for CompareExchange; ignored by other operations."]
            expected: u64,
        },
    }
}

pub use SignalCommandOperation as SignalOperation;
pub use SignalCommandOrder as SignalOrder;

impl SignalOperation {
    /// Whether this operation supports the requested memory order.
    pub const fn allows_order(self, order: SignalOrder) -> bool {
        match self {
            Self::Load => matches!(order, SignalOrder::Relaxed | SignalOrder::Acquire),
            Self::Store => matches!(order, SignalOrder::Relaxed | SignalOrder::Release),
            Self::Add
            | Self::Subtract
            | Self::And
            | Self::Or
            | Self::Xor
            | Self::Exchange
            | Self::CompareExchange => true,
        }
    }
}
