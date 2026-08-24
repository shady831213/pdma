use syn::{
    Ident, Path, Result, Token, Visibility, braced,
    parse::{Parse, ParseStream},
};

use super::{key, keyword, usize_lit};

mod field;
mod modern;

use field::parse_u16;
pub(in crate::internal) use field::{FieldDecl, FieldShape, ScalarType};
pub(in crate::internal) use modern::{
    BitsDecl, EnumDecl, ModernFieldDecl, ModernRecordDecl, ModernTypeDecl, ScopedPath, UnionDecl,
    UnionFieldDecl,
};

pub(in crate::internal) enum RecordInput {
    Legacy(RecordDecl),
    Modern(ModernRecordDecl),
}

pub(in crate::internal) struct RecordDecl {
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) size: (usize, proc_macro2::Span),
    pub(in crate::internal) alignment: (usize, proc_macro2::Span),
    pub(in crate::internal) fields: Vec<FieldDecl>,
    pub(in crate::internal) records: Vec<RecordPlacementDecl>,
}

pub(in crate::internal) struct RecordPlacementDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) definition: Path,
    pub(in crate::internal) id: (u16, proc_macro2::Span),
    pub(in crate::internal) offset: (usize, proc_macro2::Span),
}

impl Parse for RecordDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "record")?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "size")?;
        let size = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "alignment")?;
        let alignment = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "fields")?;
        let fields_body;
        braced!(fields_body in body);
        let fields = fields_body
            .parse_terminated(FieldDecl::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        let records = if body.is_empty() {
            Vec::new()
        } else {
            key(&body, "records")?;
            let records_body;
            braced!(records_body in body);
            let records = records_body
                .parse_terminated(RecordPlacementDecl::parse, Token![,])?
                .into_iter()
                .collect();
            let _ = body.parse::<Token![,]>();
            records
        };
        Ok(Self {
            visibility,
            name,
            size,
            alignment,
            fields,
            records,
        })
    }
}

impl Parse for RecordInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let probe = input.fork();
        let _visibility: Visibility = probe.parse()?;
        keyword(&probe, "record")?;
        let _name: Ident = probe.parse()?;
        let body;
        braced!(body in probe);
        let first: Ident = body.parse()?;
        if first == "size" {
            input.parse().map(Self::Legacy)
        } else {
            input.parse().map(Self::Modern)
        }
    }
}

impl Parse for RecordPlacementDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let definition = input.parse()?;
        let props;
        braced!(props in input);
        key(&props, "id")?;
        let id = parse_u16(&props)?;
        props.parse::<Token![,]>()?;
        key(&props, "offset")?;
        let offset = usize_lit(&props)?;
        Ok(Self {
            name,
            definition,
            id,
            offset,
        })
    }
}
