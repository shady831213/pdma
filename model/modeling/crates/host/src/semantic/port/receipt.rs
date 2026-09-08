use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Shared acceptance receipt retained while a transport resolves an issued command.
#[derive(Debug, Clone)]
pub struct SubmissionReceipt(Arc<ReceiptState>);

#[derive(Debug)]
struct ReceiptState {
    refundable: AtomicBool,
    pending_rollbacks: Arc<AtomicUsize>,
}

impl SubmissionReceipt {
    pub(crate) fn new(pending_rollbacks: Arc<AtomicUsize>) -> Self {
        Self(Arc::new(ReceiptState {
            refundable: AtomicBool::new(true),
            pending_rollbacks,
        }))
    }

    /// Records definitive device acceptance, preserving the reserved command credit.
    pub fn accept(&self) {
        self.issued();
    }

    /// Retains credit after command issuance, including while its result is unknown.
    pub fn issued(&self) {
        self.0.refundable.store(false, Ordering::Release);
    }

    /// Allows a refund only after definitive command rejection, never a lost response or reset.
    pub fn reject(&self) {
        self.0.refundable.store(true, Ordering::Release);
    }
}

impl Drop for ReceiptState {
    fn drop(&mut self) {
        if self.refundable.load(Ordering::Acquire) {
            self.pending_rollbacks.fetch_add(1, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refunds_once_when_an_issued_submission_is_definitively_rejected() {
        // Given: the driver and issued transport command share a reservation.
        let refunds = Arc::new(AtomicUsize::new(0));
        let driver = SubmissionReceipt::new(Arc::clone(&refunds));
        let transport = driver.clone();
        transport.issued();
        // When: cancellation drops the driver before the transport resolves rejection.
        drop(driver);
        assert_eq!(refunds.load(Ordering::Acquire), 0);
        transport.reject();
        drop(transport);
        // Then: the shared accounting receives exactly one refund.
        assert_eq!(refunds.load(Ordering::Acquire), 1);
    }

    #[test]
    fn preserves_credit_when_acceptance_is_resolved_after_driver_cancellation() {
        // Given: an issued command still owns the canceled driver's reservation.
        let refunds = Arc::new(AtomicUsize::new(0));
        let driver = SubmissionReceipt::new(Arc::clone(&refunds));
        let transport = driver.clone();
        drop(driver);
        // When: the transport observes definitive acceptance.
        transport.accept();
        drop(transport);
        // Then: completion retirement remains the sole source of returned credit.
        assert_eq!(refunds.load(Ordering::Acquire), 0);
    }
}
