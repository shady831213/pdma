use crate::memory::ContextAttributes;
use hardware_interface_schema::record_schema;

record_schema! {
    #[short_description = "Firmware control request"]
    #[description = "PF-produced 32-byte little-endian request, eight-byte aligned. The privileged management ingress authorizes the explicit target Function. DMA hardware never fetches these records."]
    pub record ControlRequest {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Host-selected request identifier echoed unchanged in the response."]
            id: u64,
            #[description = "0: AcquireChannel. 1: ReleaseChannel. 2: ResetChannel. 3: CreateContext. 4: DrainContext. 5: AbortContext. 6: DestroyContext. 7: ResetDma (all Functions). 8: ResetEvents. Other values are invalid. Context maintenance requires the documented submitter exclusion and publication ordering."]
            opcode: enum(u32) { variants: { AcquireChannel = 0, ReleaseChannel = 1, ResetChannel = 2, CreateContext = 3, DrainContext = 4, AbortContext = 5, DestroyContext = 6, ResetDma = 7, ResetEvents = 8 } },
            #[description = "Target Function in the platform-configured namespace. Only the PF management ingress may submit requests; this field is not the MMIO requester's identity. Ignored by device-wide ResetDma and ResetEvents."]
            function: u32,
            #[description = "DMA channel index for release or reset; ignored for acquisition."]
            channel: u32,
            #[description = "Independent context selector for Drain, Abort and Destroy; ignored by Create and channel operations."]
            context: u32,
            #[description = "Address-space attributes for CreateContext; ignored by other requests."]
            attributes: ContextAttributes,
        },
    }
}

record_schema! {
    #[short_description = "Firmware control response"]
    #[description = "Firmware-produced 24-byte little-endian response, eight-byte aligned. Host reads this independent control queue; hardware does not implement allocation policy."]
    pub record ControlResponse {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Identifier copied unchanged from the corresponding control request."]
            id: u64,
            #[description = "Success, resource/ownership errors, or context maintenance outcome. ContextBusy and MaintenanceFailed preserve the binding and never authorize memory reuse. InvalidAttributes rejects undefined Default/PASID combinations. DeviceFailed requires DMA recovery; InvalidState rejects a recovery operation outside its permitted lifecycle."]
            status: enum(u32) { variants: { Success = 0, NoResources = 1, InvalidChannel = 2, NotAllocated = 3, PermissionDenied = 4, InvalidFunction = 5, ContextBusy = 6, InvalidContext = 7, MaintenanceFailed = 8, InvalidAttributes = 9, DeviceFailed = 10, InvalidState = 11 } },
            #[description = "Allocated or selected channel for channel operations. Context maintenance failure identifies the first failed channel as a diagnostic, or UINT32_MAX when none applies; firmware can inspect the complete hardware failure bitmap."]
            channel: u32,
            #[description = "Created or selected context ID, independently of any recovery channel."]
            context: u32,
        },
    }
}

record_schema! {
    #[short_description = "Memory-resident control ring state"]
    #[description = "Software-owned ring geometry and wrapping entry counts. Producer follows entry storage; consumer follows entry reading. Command/response geometry stays fixed while enabled. Event geometry can change only after a retained delivery error has stopped its producer, before ResetEvents reattachment."]
    pub record ControlRingState {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Memory address of the entry buffer."]
            base: u64,
            #[description = "Nonzero power-of-two entry capacity below 2^31, validated by software."]
            capacity: u32,
            #[description = "Wrapping published-entry count, owned by the producer."]
            producer: u32,
            #[description = "Wrapping consumed-entry count, owned by the consumer."]
            consumer: u32,
        },
    }
}

record_schema! {
    #[short_description = "Memory-resident firmware control queue"]
    #[description = "Host allocates and initializes all three rings before publishing this record's address through bootstrap. Each software side updates only its owned cursor fields."]
    pub record ControlQueue {
        alignment: 8, endian: Little,
        fields: {
            #[description = "Host-produced and firmware-consumed control requests."]
            command: ControlRingState,
            #[description = "Firmware-produced and Host-consumed control responses."]
            response: ControlRingState,
            #[description = "Firmware-produced PF asynchronous events, independent of request completion."]
            event: ControlRingState,
        },
    }
}

record_schema! {
    #[short_description = "Asynchronous PF event"]
    #[description = "Firmware-produced event with no request ID. Delivery failure is retained in event_error, without retries or backpressure."]
    pub record ControlEvent {
        alignment: 4, endian: Little,
        fields: {
            #[description = "0: DmaFatal, observed by firmware. Other kinds are invalid."]
            kind: enum(u32) { variants: { DmaFatal = 0 } },
            #[description = "DMA fatal code captured by firmware."]
            code: u32,
        },
    }
}
