use super::signal::SignalTarget;
use hardware_interface_schema::record_schema;

record_schema! {
    #[short_description = "Signal wait command"]
    #[description = "Wait until one atomic signal load satisfies a signed comparison. An unmet condition retains the command while peer channels remain eligible."]
    pub record WaitCommand {
        alignment: 8, endian: Little,
        fields: {
            #[short_description = "Signal target"]
            #[description = "Address and context selecting the signal allocation."]
            target: SignalTarget,
            #[short_description = "Signed comparison"]
            #[description = "Compare the observed signed 64-bit value with compare using Equal, NotEqual, LessThan or GreaterOrEqual."]
            condition: enum(u32) { variants: {
                Equal = 0, NotEqual = 1, LessThan = 2, GreaterOrEqual = 3,
            } },
            #[short_description = "Load memory order"]
            #[description = "Relaxed or Acquire ordering for every atomic signal observation."]
            order: enum(u32) { variants: { Relaxed = 0, Acquire = 1 } },
            #[short_description = "Comparison value"]
            #[description = "Signed comparison operand stored as two's-complement u64 bits."]
            compare: u64,
        },
    }
}

pub use WaitCommandCondition as WaitCondition;
pub use WaitCommandOrder as WaitOrder;

impl WaitCondition {
    /// Tests the signed observation against the requested threshold.
    pub const fn matches(self, observed: i64, compare: i64) -> bool {
        match self {
            Self::Equal => observed == compare,
            Self::NotEqual => observed != compare,
            Self::LessThan => observed < compare,
            Self::GreaterOrEqual => observed >= compare,
        }
    }
}
