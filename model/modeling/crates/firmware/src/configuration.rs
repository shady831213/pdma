use pdma_model_software::control::ManagementAddresses;

/// Platform-provided firmware register locations and trusted control-ingress count.
#[derive(Clone, Copy, Debug)]
pub struct Configuration {
    /// Bases of the shared DMA, permissions, boot status and control bank.
    pub addresses: ManagementAddresses,
    /// Control-bank slots identify Functions zero through count minus one.
    pub function_count: usize,
}

impl From<ManagementAddresses> for Configuration {
    fn from(addresses: ManagementAddresses) -> Self {
        Self {
            addresses,
            function_count: 1,
        }
    }
}
