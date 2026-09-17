use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
};
use std::{
    future::pending,
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};

#[derive(Debug, Default)]
/// Exact cursor transport with deterministic suspension around publication.
pub struct CursorBus {
    pub(super) memory: SharedMemory,
    pub(super) pause: AtomicU8,
    pub(super) writes: AtomicUsize,
}

impl RegisterAccess for CursorBus {
    type Error = MemoryAccessError;
    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        assert!(offset == 0 || offset == 4);
        assert_eq!(response.len(), 4);
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
