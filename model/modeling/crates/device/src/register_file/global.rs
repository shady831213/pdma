use super::CommandBank;
use pdma_model_contract::abi::mmio::GlobalResponseBank;

#[derive(Debug)]
pub(crate) struct GlobalBank {
    pub(crate) req_abi_version: u32,
    pub(crate) req_features: u64,
    pub(crate) reset_expected_epoch: u64,
    pub(crate) command: CommandBank<GlobalResponseBank>,
}

impl GlobalBank {
    pub(crate) fn new() -> Self {
        Self {
            req_abi_version: 0,
            req_features: 0,
            reset_expected_epoch: 0,
            command: CommandBank::new(),
        }
    }
}
