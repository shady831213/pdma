use hardware_interface_schema::export::backends::c_header::{
    RecordCHeaderModel, RegisterMapCHeaderModel,
};

use super::super::{
    CompletionSchema, DescriptorSchema, EventSchema, IndirectDestinationEntrySchema,
    mmio::{PdmaMmioMap, PdmaMmioMapRegisterId},
};

pub(super) struct GenericFragments {
    pub(super) records: [RecordCHeaderModel; 4],
    pub(super) mmio: RegisterMapCHeaderModel<PdmaMmioMapRegisterId>,
}

pub(super) fn render_fragments() -> GenericFragments {
    GenericFragments {
        records: [
            RecordCHeaderModel::from_spec(DescriptorSchema::SPEC),
            RecordCHeaderModel::from_spec(CompletionSchema::SPEC),
            RecordCHeaderModel::from_spec(EventSchema::SPEC),
            RecordCHeaderModel::from_spec(IndirectDestinationEntrySchema::SPEC),
        ],
        mmio: RegisterMapCHeaderModel::from_spec(PdmaMmioMap::SPEC),
    }
}
