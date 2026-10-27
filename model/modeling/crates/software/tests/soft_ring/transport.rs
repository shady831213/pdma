use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_io::memory::{MemoryAccessError, SharedMemory};
use hardware_interface_io::register::RegisterAccess;
use std::{
    future::pending,
    sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
};

#[derive(Debug, Default)]
/// Exact cursor transport with deterministic suspension around publication.
pub struct CursorBus {
    pub(super) memory: SharedMemory,
    pub(super) pause: AtomicU8,
    pub(super) pause_read: AtomicBool,
    pub(super) reads: AtomicUsize,
    pub(super) writes: AtomicUsize,
}

impl RegisterAccess for CursorBus {
    type Error = MemoryAccessError;
    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        assert!(offset == 0 || offset == 4);
        assert_eq!(response.len(), 4);
        if self.pause_read.swap(false, Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.memory.read(
            MemoryAddress::new(u64::from(offset)),
            MemoryAlignment::ONE,
            response,
        )
    }
    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        assert!(offset == 0 || offset == 4);
        assert_eq!(request.len(), 4);
        let pause = self.pause.swap(0, Ordering::SeqCst);
        if pause == 1 {
            pending::<()>().await;
        }
        if pause == 3 {
            tokio::task::yield_now().await;
        }
        self.memory.write(
            MemoryAddress::new(u64::from(offset)),
            MemoryAlignment::ONE,
            request,
        )?;
        self.writes.fetch_add(1, Ordering::SeqCst);
        if pause == 2 {
            pending::<()>().await;
        }
        Ok(())
    }
}
