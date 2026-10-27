use super::{COPY_CHUNK_BYTES, CopyState};
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress};
use pdma_model_contract::dma::EventCode;

impl CopyState {
    pub(crate) fn prepare<M: MemoryAccess>(
        &mut self,
        memory: &M,
    ) -> Option<(MemoryAddress, MemoryAddress, usize)> {
        let progress = self;
        let mut source = 0;
        let mut destination = 0;
        let mut length = 0;
        match progress.status {
            Ok(()) if progress.remaining > 0 => {
                let cursor = if progress.source.remaining() == 0 {
                    Some(&mut progress.source)
                } else if progress.destination.remaining() == 0 {
                    Some(&mut progress.destination)
                } else {
                    None
                };
                if let Some(cursor) = cursor {
                    match cursor.next_segment(memory) {
                        Ok(true) => {}
                        Ok(false) => progress.status = Err(EventCode::LengthMismatch),
                        Err(status) => progress.status = Err(status),
                    }
                    return None;
                }
                let Some(address) = progress.source.address() else {
                    progress.status = Err(EventCode::ReadError);
                    return None;
                };
                source = address;
                let Some(address) = progress.destination.address() else {
                    progress.status = Err(EventCode::WriteError);
                    return None;
                };
                destination = address;
                let remaining = progress
                    .remaining
                    .min(progress.source.remaining())
                    .min(progress.destination.remaining());
                length = usize::try_from(remaining)
                    .unwrap_or(COPY_CHUNK_BYTES)
                    .min(COPY_CHUNK_BYTES);
            }
            Ok(()) | Err(_) => {}
        }

        Some((
            MemoryAddress::new(source),
            MemoryAddress::new(destination),
            length,
        ))
    }
}
