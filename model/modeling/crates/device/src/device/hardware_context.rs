use crate::dma::PdmaMemoryMappings;
use crate::trace::TraceSink;

pub(crate) struct HardwareContext<'a> {
    pub(crate) dma: &'a PdmaMemoryMappings,
    pub(crate) trace: &'a mut TraceSink,
    pub(crate) fetches: &'a mut usize,
}
