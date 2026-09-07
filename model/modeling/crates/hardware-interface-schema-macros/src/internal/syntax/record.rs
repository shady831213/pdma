mod modern;
mod scalar;

pub(in crate::internal) use modern::{
    BitsDecl, EnumDecl, ModernFieldDecl, ModernRecordDecl, ModernTypeDecl, ScopedPath,
    UnionCaseDecl, UnionDecl, UnionFieldDecl, UnionFieldType,
};
pub(in crate::internal) use scalar::ScalarType;
