use super::QueueBank;
use pdma_model_contract::abi::mmio::{
    MmioResult, MmioResultDetails, QueueResponseBank, ResponseUpdate,
};

#[derive(Clone, Copy)]
pub(in crate::runner) struct BootstrapBatchStaging {
    pub(in crate::runner) expected_epoch: u64,
    pub(in crate::runner) id: u64,
    pub(in crate::runner) cfg_software_owner: u64,
    pub(in crate::runner) sq_base: u64,
    pub(in crate::runner) sq_depth: u32,
    pub(in crate::runner) cq_base: u64,
    pub(in crate::runner) cq_depth: u32,
    pub(in crate::runner) eq_base: u64,
    pub(in crate::runner) eq_depth: u32,
}

#[derive(Clone, Copy)]
pub(in crate::runner) struct SubmissionBatchStaging {
    pub(in crate::runner) expected_epoch: u64,
    pub(in crate::runner) id: u64,
    pub(in crate::runner) expected_cursor: u64,
    pub(in crate::runner) new_cursor: u64,
}

impl QueueBank {
    pub(in crate::runner) fn complete_bootstrap_batch(
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

    pub(in crate::runner) fn complete_submission_batch(
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
