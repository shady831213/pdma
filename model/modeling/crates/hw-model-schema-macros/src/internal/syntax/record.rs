mod modern;
mod scalar;

pub(in crate::internal) use modern::{
    BitsDecl, ModernFieldDecl, ModernRecordDecl, ModernTypeDecl, TaggedUnionCaseDecl,
    TaggedUnionDecl, TaggedUnionFieldDecl, TaggedUnionFieldType, TaggedUnionTagPath,
};
pub(in crate::internal) use scalar::ScalarType;
