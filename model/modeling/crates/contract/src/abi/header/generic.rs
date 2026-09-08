use hardware_interface_schema::ResolvedRecord;
use hardware_interface_schema::export::backends::c_header::RecordCHeaderModel;

use super::PdmaHeaderSchemas;

pub(super) struct GenericFragments {
    pub(super) records: [RecordCHeaderModel; 4],
}

pub(super) fn project(schemas: &PdmaHeaderSchemas) -> GenericFragments {
    let project_record = |record: &ResolvedRecord| RecordCHeaderModel::from_resolved(record);
    GenericFragments {
        records: schemas.records.each_ref().map(project_record),
    }
}
