use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{Command, WordTargetWidth, WordWrite};

/// A software-selected completion value in storage retained until retirement.
#[derive(Clone, Copy, Debug)]
pub struct CompletionWord(pub WordWrite);

impl CompletionWord {
    /// Builds a Fence without allocating or initializing its storage.
    pub const fn command(self, id: u64) -> Command {
        Command::fence(id, self.0)
    }

    /// Observes the Fence value; callers must also handle error, fatal and reset exits.
    pub fn is_complete<M: MemoryAccess>(&self, memory: &M) -> Result<bool, M::Error> {
        let address = MemoryAddress::new(self.0.target.address);
        match self.0.target.width {
            WordTargetWidth::Bits32 => {
                let mut bytes = [0; 4];
                memory.read(address, MemoryAlignment::ONE, &mut bytes)?;
                Ok(u64::from(u32::from_le_bytes(bytes)) == (self.0.value & u64::from(u32::MAX)))
            }
            WordTargetWidth::Bits64 => {
                let mut bytes = [0; 8];
                memory.read(address, MemoryAlignment::ONE, &mut bytes)?;
                Ok(u64::from_le_bytes(bytes) == self.0.value)
            }
        }
    }
}
