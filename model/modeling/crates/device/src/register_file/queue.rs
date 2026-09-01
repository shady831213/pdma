use super::CommandBank;
use pdma_model_contract::abi::mmio::QueueResponseBank;

#[derive(Debug)]
pub(crate) struct QueueBank {
    pub(crate) expected_epoch: u64,
    pub(crate) id: u64,
    pub(crate) expected_cursor: u64,
    pub(crate) new_cursor: u64,
    pub(crate) cfg_sq_depth: u32,
    pub(crate) cfg_cq_depth: u32,
    pub(crate) cfg_software_owner: u64,
    pub(crate) sq_base: u64,
    pub(crate) sq_depth: u32,
    pub(crate) cq_base: u64,
    pub(crate) cq_depth: u32,
    pub(crate) eq_base: u64,
    pub(crate) eq_depth: u32,
    pub(crate) command: CommandBank<QueueResponseBank>,
}

#[derive(Clone, Copy)]
pub(crate) struct QueueStaging {
    pub(crate) expected_epoch: u64,
    pub(crate) id: u64,
    pub(crate) expected_cursor: u64,
    pub(crate) new_cursor: u64,
    pub(crate) cfg_sq_depth: u32,
    pub(crate) cfg_cq_depth: u32,
    pub(crate) cfg_software_owner: u64,
    pub(crate) sq_base: u64,
    pub(crate) sq_depth: u32,
    pub(crate) cq_base: u64,
    pub(crate) cq_depth: u32,
    pub(crate) eq_base: u64,
    pub(crate) eq_depth: u32,
}

impl QueueBank {
    pub(crate) fn new() -> Self {
        Self {
            expected_epoch: 0,
            id: 0,
            expected_cursor: 0,
            new_cursor: 0,
            cfg_sq_depth: 0,
            cfg_cq_depth: 0,
            cfg_software_owner: 0,
            sq_base: 0,
            sq_depth: 0,
            cq_base: 0,
            cq_depth: 0,
            eq_base: 0,
            eq_depth: 0,
            command: CommandBank::new(),
        }
    }

    pub(crate) const fn staging(&self) -> QueueStaging {
        QueueStaging {
            expected_epoch: self.expected_epoch,
            id: self.id,
            expected_cursor: self.expected_cursor,
            new_cursor: self.new_cursor,
            cfg_sq_depth: self.cfg_sq_depth,
            cfg_cq_depth: self.cfg_cq_depth,
            cfg_software_owner: self.cfg_software_owner,
            sq_base: self.sq_base,
            sq_depth: self.sq_depth,
            cq_base: self.cq_base,
            cq_depth: self.cq_depth,
            eq_base: self.eq_base,
            eq_depth: self.eq_depth,
        }
    }
}
