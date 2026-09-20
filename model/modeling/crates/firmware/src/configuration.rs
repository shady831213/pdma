use pdma_model_software::control::ManagementAddresses;

/// Platform-provided firmware register locations and valid target Function count.
#[derive(Clone, Copy, Debug)]
pub struct Configuration {
    /// Bases of the shared DMA, permissions, boot status and PF control channel.
    pub addresses: ManagementAddresses,
    /// Valid PF request targets are Functions zero through count minus one.
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
