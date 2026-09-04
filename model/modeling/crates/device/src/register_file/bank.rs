use pdma_model_contract::abi::mmio::{decode_u32, encode_u64};

pub(crate) struct MmioScalar(Vec<u8>);

impl MmioScalar {
    pub(crate) const fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub(crate) fn encode(self) -> Vec<u8> {
        self.0
    }
}

pub(crate) const fn narrow_u32(value: u64) -> u32 {
    let [byte0, byte1, byte2, byte3, _, _, _, _] = encode_u64(value);
    decode_u32([byte0, byte1, byte2, byte3])
}
