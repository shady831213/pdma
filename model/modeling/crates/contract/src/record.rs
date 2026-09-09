use hardware_interface_schema::record_schema;

record_schema! {
    #[short_description = "DMA copy command"]
    #[description = "A software-produced command ring entry requesting one bounded memory copy. The wire record is 32 bytes, eight-byte aligned and little-endian; its trailing four padding bytes encode as zero."]
    pub record Command {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Command identifier"]
            #[description = "Software-selected identifier echoed unchanged in the response. Hardware does not interpret or enforce uniqueness of this value."]
            id: u64,
            #[short_description = "Source address"]
            #[description = "First payload byte to read in the attached memory backend's address space. No additional payload alignment is required. The address is not accessed for a zero-length or oversized command."]
            source: u64,
            #[short_description = "Destination address"]
            #[description = "First payload byte to write in the attached memory backend's address space. Hardware retains the complete source payload before writing, so overlapping source and destination ranges are supported."]
            destination: u64,
            #[short_description = "Copy length in bytes"]
            #[description = "Valid lengths are 0 through 256. Zero succeeds without payload memory access. A larger value produces InvalidLength without reading or writing payload memory; validation occurs during execution, not wire decoding."]
            length: u32,
        },
    }
}

record_schema! {
    #[short_description = "DMA copy response"]
    #[description = "A hardware-produced response ring entry carrying one command's result. The wire record is 16 bytes, eight-byte aligned and little-endian, with four zero padding bytes. Ring-access faults instead stop the channel and are reported through its fault register and IRQ condition."]
    pub record Response {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Completed command identifier"]
            #[description = "The identifier copied unchanged from the command, allowing software to associate this response with its submission."]
            id: u64,
            #[short_description = "Copy result"]
            #[description = "0: Success, including a zero-length copy.\n1: InvalidLength, exceeding the 256-byte limit.\n2: ReadFault, the payload source could not be read.\n3: WriteFault, the payload destination could not be written.\nOther wire values are invalid. Payload errors return a response rather than setting the sticky ring-access fault."]
            status: enum(u32) {
                variants: {
                    Success = 0,
                    InvalidLength = 1,
                    ReadFault = 2,
                    WriteFault = 3,
                },
            },
        },
    }
}
