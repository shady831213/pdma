/// One decoded DMA result cached before result-ring publication is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingDmaResult {
    /// Successful execution waiting for completion-ring publication.
    Completion(pdma_model_contract::abi::AbiCompletion),
    /// Canonical submission fault waiting for event-ring publication.
    Event(pdma_model_contract::abi::AbiEvent),
}
