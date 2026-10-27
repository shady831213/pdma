use pdma_model_contract::dma::{
    CopyCommandDestinationValues, CopyCommandSourceValues, LinearEndpoint,
};

use super::*;

pub(in crate::harness::program::contexts) struct Mixed {
    target: MemoryAddress,
    peer: MemoryAddress,
    active: MemoryAddress,
    pub(in crate::harness::program::contexts) partial: usize,
}

impl Transfer {
    pub(in crate::harness::program::contexts) async fn mixed(
        &mut self,
        spaces: &Spaces,
        (contexts, first): ([ContextId; 4], u64),
    ) -> TestResult<Mixed> {
        let length = 65_536;
        let source = spaces.source.allocate(length, MemoryAlignment::ONE)?;
        let destination = spaces.destination.allocate(length, MemoryAlignment::ONE)?;
        spaces.destination.memory_access().write(
            destination,
            MemoryAlignment::ONE,
            &vec![0; usize::try_from(length)?],
        )?;
        spaces.source.memory_access().write(
            source,
            MemoryAlignment::ONE,
            &vec![0x6d; usize::try_from(length)?],
        )?;
        self.driver
            .submit(command(
                first,
                ((source, contexts[2]), (destination, contexts[3])),
                length,
            ))
            .await?;
        let mut bytes = vec![0; usize::try_from(length)?];
        let partial = loop {
            spaces.destination.memory_access().read(
                destination,
                MemoryAlignment::ONE,
                &mut bytes,
            )?;
            let copied = bytes.iter().take_while(|&&byte| byte == 0x6d).count();
            if copied > 0 {
                break copied;
            }
            tokio::task::yield_now().await;
        };
        let target = spaces.destination.allocate(4, MemoryAlignment::ONE)?;
        let peer = spaces.destination.allocate(4, MemoryAlignment::ONE)?;
        spaces
            .destination
            .memory_access()
            .write(target, MemoryAlignment::ONE, &[0xa5; 4])?;
        spaces
            .destination
            .memory_access()
            .write(peer, MemoryAlignment::ONE, &[0; 4])?;
        let peer_source = spaces.lists.allocate(4, MemoryAlignment::ONE)?;
        spaces
            .lists
            .memory_access()
            .write(peer_source, MemoryAlignment::ONE, b"peer")?;
        self.driver
            .submit(command(
                first + 1,
                ((source, contexts[2]), (target, contexts[3])),
                4,
            ))
            .await?;
        self.driver
            .submit(command(
                first + 2,
                ((peer_source, contexts[0]), (peer, contexts[3])),
                4,
            ))
            .await?;
        self.fence.0.value = first + 2;
        self.driver.submit(self.fence.command(first + 3)).await?;
        Ok(Mixed {
            target,
            peer,
            partial,
            active: destination,
        })
    }
}

impl Mixed {
    pub(in crate::harness::program::contexts) fn bytes(
        &self,
        spaces: &Spaces,
    ) -> TestResult<([u8; 4], [u8; 4])> {
        let memory = spaces.destination.memory_access();
        let mut active = vec![0; 65_536];
        memory.read(self.active, MemoryAlignment::ONE, &mut active)?;
        if active.iter().any(|byte| *byte != 0x6d) {
            return Err("active command did not finish before Fence".into());
        }
        Ok((
            memory.read_buffer(self.target, MemoryAlignment::ONE)?,
            memory.read_buffer(self.peer, MemoryAlignment::ONE)?,
        ))
    }
}

const fn command(
    id: u64,
    (source, destination): ((MemoryAddress, ContextId), (MemoryAddress, ContextId)),
    length: u64,
) -> Command {
    Command::Copy(pdma_model_contract::dma::CopyCommand {
        id,
        copy_length: length,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: source.0.raw(),
            offset: 0,
        }),
        source_context: source.1.raw(),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: destination.0.raw(),
            offset: 0,
        }),
        destination_context: destination.1.raw(),
        sg_context: 0,
        ..pdma_model_contract::dma::CopyCommand::ZERO
    })
}
