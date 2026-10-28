use super::{FlowResult, byte_hash, require};
use crate::run_adverse_dispatch;
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hw_model_io::memory::{MemoryAccessError, MemoryRegion, SharedMemory};

pub(super) async fn run(
    memory: &SharedMemory,
    region: MemoryRegion,
    alignment: MemoryAlignment,
) -> FlowResult<String> {
    check_memory(memory, region, alignment)?;
    run_adverse_dispatch().await?;
    Ok(String::from(
        "adverse memory=Unmapped{address=0x5080},CrossesRegion{address=0x503f,length=2,region=0x5000..0x5040};destination_hash=120;store_hash=5835;interrupt=UnregisteredVector(9);handler_entries=0;env_entries=0;lifecycle_events=0;source_consumed=1;source_drops=1;handler_drops=1;joined=true",
    ))
}

fn check_memory(
    memory: &SharedMemory,
    region: MemoryRegion,
    alignment: MemoryAlignment,
) -> FlowResult<()> {
    let mut destination = [0x31, 0x47];
    let before = destination;
    let unmapped = memory.read(MemoryAddress::new(0x5080), alignment, &mut destination);
    let crossing = memory.write(
        MemoryAddress::new(0x503f),
        MemoryAlignment::try_new(1)?,
        &[0xaa, 0xbb],
    );
    require(
        unmapped
            == Err(MemoryAccessError::Unmapped {
                address: MemoryAddress::new(0x5080),
            }),
        "unmapped error changed",
    )?;
    require(
        crossing
            == Err(MemoryAccessError::CrossesRegion {
                address: MemoryAddress::new(0x503f),
                length: 2,
                region,
            }),
        "crossing error changed",
    )?;
    require(
        destination == before && byte_hash(&destination) == 120,
        "unmapped read changed destination",
    )?;
    let mut snapshot = [0_u8; 64];
    memory.read(MemoryAddress::new(0x5000), alignment, &mut snapshot)?;
    require(byte_hash(&snapshot) == 5835, "store hash changed")?;
    Ok(())
}
