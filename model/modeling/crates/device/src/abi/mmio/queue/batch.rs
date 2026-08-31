//! Atomic queue MMIO batch staging.

use super::QueueBank;
use pdma_model_contract::abi::mmio::{
    MmioResult, MmioResultDetails, QueueResponseBank, ResponseUpdate,
};

#[derive(Clone, Copy)]
pub(crate) struct BootstrapBatchStaging {
    pub(crate) expected_epoch: u64,
    pub(crate) id: u64,
    pub(crate) cfg_software_owner: u64,
    pub(crate) sq_base: u64,
    pub(crate) sq_depth: u32,
    pub(crate) cq_base: u64,
    pub(crate) cq_depth: u32,
    pub(crate) eq_base: u64,
    pub(crate) eq_depth: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct SubmissionBatchStaging {
    pub(crate) expected_epoch: u64,
    pub(crate) id: u64,
    pub(crate) expected_cursor: u64,
    pub(crate) new_cursor: u64,
}

impl QueueBank {
    pub(crate) fn complete_bootstrap_batch(
        &mut self,
        staging: BootstrapBatchStaging,
        response: QueueResponseBank,
    ) {
        self.expected_epoch = staging.expected_epoch;
        self.id = staging.id;
        self.expected_cursor = 0;
        self.new_cursor = 0;
        self.cfg_sq_depth = staging.sq_depth;
        self.cfg_cq_depth = staging.cq_depth;
        self.cfg_software_owner = staging.cfg_software_owner;
        self.sq_base = staging.sq_base;
        self.sq_depth = staging.sq_depth;
        self.cq_base = staging.cq_base;
        self.cq_depth = staging.cq_depth;
        self.eq_base = staging.eq_base;
        self.eq_depth = staging.eq_depth;
        self.complete_batch(response);
    }

    pub(crate) fn complete_submission_batch(
        &mut self,
        staging: SubmissionBatchStaging,
        response: QueueResponseBank,
    ) {
        self.expected_epoch = staging.expected_epoch;
        self.id = staging.id;
        self.expected_cursor = staging.expected_cursor;
        self.new_cursor = staging.new_cursor;
        self.complete_batch(response);
    }

    fn complete_batch(&mut self, response: QueueResponseBank) {
        self.command.complete(
            MmioResultDetails {
                result: MmioResult::Ok,
                details: [0; 3],
            },
            ResponseUpdate::Replace(response),
        );
    }
}
