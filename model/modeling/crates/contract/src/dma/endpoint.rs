use hw_model_schema::record_schema;

record_schema! {
    #[short_description = "Scatter/gather segment"]
    #[description = "One contiguous payload span in a software-owned scatter/gather list. The 16-byte little-endian record is eight-byte aligned. Software keeps lists and payload buffers stable until the command completes."]
    pub record Segment {
        alignment: 8,
        endian: Little,
        fields: {
            #[short_description = "Payload address"]
            #[description = "First payload byte in the attached memory address space. Zero-length segments perform no payload access and require no usable address."]
            address: u64,
            #[short_description = "Segment byte length"]
            #[description = "Number of bytes contributed to the endpoint stream. Zero-length segments are skipped. Hardware checks arithmetic only for the selected copy range."]
            length: u64,
        },
    }
}

record_schema! {
    #[short_description = "Linear copy endpoint"]
    #[description = "Base address and full-width byte offset in the selected payload context."]
    pub record LinearEndpoint { alignment: 8, endian: Little, fields: {
        #[short_description = "Payload base"] #[description = "Payload base address."]
        address: u64,
        #[short_description = "Byte offset"] #[description = "Full-width base-relative byte offset."]
        offset: u64,
    } }
}
record_schema! {
    #[short_description = "Scatter/gather copy endpoint"]
    #[description = "Original list geometry and full-width starting position. Lists remain stable until retirement."]
    pub record ScatterGatherEndpoint { alignment: 8, endian: Little, fields: {
        #[short_description = "Original table address"] #[description = "Original table address in the SG context."]
        list_address: u64,
        #[short_description = "Total entry count"] #[description = "Full-width total entry count."]
        count: u64,
        #[short_description = "Starting entry"] #[description = "Full-width original table entry index."]
        index: u64,
        #[short_description = "Entry offset"] #[description = "Full-width entry-local byte offset."]
        offset: u64,
    } }
}
