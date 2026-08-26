use super::{global::GlobalBank, irq::IrqBank, queue::QueueBank};
use pdma_model_contract::abi::mmio::{
    MmioResult, MmioResultDetails, ResponseUpdate, apply_response_update, decode_u32, encode_u32,
    encode_u64,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runner) enum MmioScalar {
    U32(u32),
    U64(u64),
}

impl MmioScalar {
    pub(in crate::runner) fn encode(self) -> Vec<u8> {
        match self {
            Self::U32(value) => encode_u32(value).to_vec(),
            Self::U64(value) => encode_u64(value).to_vec(),
        }
    }
}

pub(super) const fn narrow_u32(value: u64) -> u32 {
    let [byte0, byte1, byte2, byte3, _, _, _, _] = encode_u64(value);
    decode_u32([byte0, byte1, byte2, byte3])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CommandBank<R> {
    pub(super) result: MmioResultDetails,
    pub(super) response: R,
}

impl<R: Copy + Default> CommandBank<R> {
    pub(super) fn new() -> Self {
        Self {
            result: MmioResultDetails {
                result: MmioResult::NotExecuted,
                details: [0; 3],
            },
            response: R::default(),
        }
    }

    pub(super) fn complete(&mut self, result: MmioResultDetails, update: ResponseUpdate<R>) {
        *self = Self {
            result,
            response: apply_response_update(self.response, update),
        };
    }
}

#[derive(Debug)]
pub(in crate::runner) struct RegisterBanks {
    pub(super) global: GlobalBank,
    pub(in crate::runner) queue: QueueBank,
    pub(super) irq: IrqBank,
}

impl RegisterBanks {
    pub(in crate::runner) fn new() -> Self {
        Self {
            global: GlobalBank::new(),
            queue: QueueBank::new(),
            irq: IrqBank::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_completion_replaces_or_preserves_atomically() {
        // Given: a prior successful response and a fresh result latch.
        let mut bank = CommandBank::<u64>::new();
        let ok = MmioResultDetails {
            result: MmioResult::Ok,
            details: [1, 2, 3],
        };
        bank.complete(ok, ResponseUpdate::Replace(0xfeed));

        // When: a later semantic failure updates only its result and details.
        let failed = MmioResultDetails {
            result: MmioResult::InvalidTransaction,
            details: [4, 5, 6],
        };
        bank.complete(failed, ResponseUpdate::Preserve);

        // Then: the result is replaced while the successful response is preserved.
        assert_eq!(bank.result, failed);
        assert_eq!(bank.response, 0xfeed);
    }

    #[test]
    fn empty_completion_zero_fills_the_response() {
        // Given: a bank containing a prior response.
        let mut bank = CommandBank::<u64>::new();
        bank.response = 0xfeed;

        // When: an EMPTY command completion is applied.
        let empty = MmioResultDetails {
            result: MmioResult::Empty,
            details: [0; 3],
        };
        bank.complete(empty, ResponseUpdate::Empty);

        // Then: result and response change as one complete bank value.
        assert_eq!(bank.result, empty);
        assert_eq!(bank.response, 0);
    }
}
