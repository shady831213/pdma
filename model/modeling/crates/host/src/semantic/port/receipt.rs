use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Shared command-credit receipt accepted by a successful producer write.
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
        self.0.refundable.store(false, Ordering::Release);
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
    fn refunds_once_when_the_last_unaccepted_receipt_is_dropped() {
        // Given: the driver and its port share an unaccepted reservation.
        let refunds = Arc::new(AtomicUsize::new(0));
        let driver = SubmissionReceipt::new(Arc::clone(&refunds));
        let port = driver.clone();
        // When: cancellation drops both receipt owners before producer publication.
        drop(driver);
        assert_eq!(refunds.load(Ordering::Acquire), 0);
        drop(port);
        // Then: the shared accounting receives exactly one refund.
        assert_eq!(refunds.load(Ordering::Acquire), 1);
    }

    #[test]
    fn preserves_credit_when_a_shared_receipt_accepts_producer_publication() {
        // Given: the port still owns the cancelled driver's reservation.
        let refunds = Arc::new(AtomicUsize::new(0));
        let driver = SubmissionReceipt::new(Arc::clone(&refunds));
        let port = driver.clone();
        drop(driver);
        assert_eq!(refunds.load(Ordering::Acquire), 0);
        // When: the producer write accepts the command before the final receipt is dropped.
        port.accept();
        drop(port);
        // Then: completion retirement remains the sole source of returned credit.
        assert_eq!(refunds.load(Ordering::Acquire), 0);
    }
}
