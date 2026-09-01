use super::CommandBank;
use pdma_model_contract::abi::mmio::IrqResponseBank;

#[derive(Debug)]
pub(crate) struct IrqBank {
    pub(crate) expected_epoch: u64,
    pub(crate) source_id: u64,
    pub(crate) source_kind: u32,
    pub(crate) route_kind: u32,
    pub(crate) route_vector: u64,
    pub(crate) command: CommandBank<IrqResponseBank>,
}

impl IrqBank {
    pub(crate) fn new() -> Self {
        Self {
            expected_epoch: 0,
            source_id: 0,
            source_kind: 0,
            route_kind: 0,
            route_vector: 0,
            command: CommandBank::new(),
        }
    }
}
