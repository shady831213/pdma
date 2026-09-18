use pdma_model_software::control::ManagementAddresses;

#[doc = "Register placement selected by the firmware test platform."]
pub const ADDRESSES: ManagementAddresses = ManagementAddresses {
    dma: 0,
    windows: 0x2000,
    control: 0x3000,
    firmware: 0x4000,
};
