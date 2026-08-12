use hardware_interface_schema::ResolvedRecord;
use hardware_interface_schema::export::backends::c_header::{
    RecordCHeaderModel, RegisterMapCHeaderModel,
};

use super::{super::mmio::PdmaMmioMapRegisterId, PdmaHeaderSchemas};

pub(super) struct GenericFragments {
    pub(super) records: [RecordCHeaderModel; 4],
    pub(super) mmio: RegisterMapCHeaderModel<PdmaMmioMapRegisterId>,
}

pub(super) fn project(schemas: &PdmaHeaderSchemas) -> GenericFragments {
    let project_record = |record: &ResolvedRecord| RecordCHeaderModel::from_resolved(record);
    GenericFragments {
        records: schemas.records.each_ref().map(project_record),
        mmio: RegisterMapCHeaderModel::from_resolved(&schemas.mmio),
    }
}
