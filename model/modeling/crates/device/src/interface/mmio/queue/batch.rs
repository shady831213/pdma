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
