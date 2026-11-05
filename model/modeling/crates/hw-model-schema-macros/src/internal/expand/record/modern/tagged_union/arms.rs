use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;

use crate::internal::syntax::{ModernRecordDecl, TaggedUnionDecl};

use super::super::layout::TaggedUnionStorage;
use super::{case_field, case_type};

pub(super) struct Context<'a> {
    pub(super) record: &'a ModernRecordDecl,
    pub(super) field: &'a syn::Ident,
    pub(super) decl: &'a TaggedUnionDecl,
    pub(super) tag_type: &'a TokenStream,
    pub(super) storage: &'a TaggedUnionStorage,
    pub(super) slots: usize,
    pub(super) quantum: &'a TokenStream,
    pub(super) facade: &'a TokenStream,
    pub(super) endian: &'a syn::Ident,
}

pub(super) fn decode(context: &Context<'_>) -> Vec<TokenStream> {
    let record_marker = &context.record.name;
    let union_identity = format_ident!(
        "{}_IDENTITY",
        context.field.unraw().to_string().to_ascii_uppercase()
    );
    let storage_type = &context.storage.ty;
    context
        .decl
        .cases
        .iter()
        .map(|case| {
            let case_name = &case.name;
            let case_type = case_type(context.record, context.field, case_name);
            let reserved = (case.fields.len()..context.slots).map(|index| {
                let quantum = context.quantum;
                let relative = quote!((#index).saturating_mul(#quantum));
                let facade = context.facade;
                let endian = context.endian;
                quote!({
                    let offset = base.checked_add(#relative).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
                    let end = offset.checked_add(#quantum).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
                    let field = bytes.get(offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
                    let value: #storage_type = #facade::read_scalar(field, #facade::Endian::#endian)?;
                    let observed = u64::from(value);
                    if observed != 0 {
                        return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved {
                            location: #record_marker::#union_identity.at(offset),
                            observed,
                        });
                    }
                })
            });
            let fields = case.fields.iter().enumerate().map(|(index, field)| {
                let quantum = context.quantum;
                let relative = quote!((#index).saturating_mul(#quantum));
                case_field::decode(case_field::Context {
                    owner: &case_type,
                    field,
                    relative,
                    quantum: context.quantum,
                    facade: context.facade,
                    endian: context.endian,
                })
            });
            let tag_type = context.tag_type;
            quote!(#tag_type::#case_name => {
                #(#reserved)*
                ::core::result::Result::Ok(Self::#case_name(#case_type { #(#fields,)* }))
            })
        })
        .collect()
}

pub(super) fn encode(context: &Context<'_>) -> Vec<TokenStream> {
    context
        .decl
        .cases
        .iter()
        .map(|case| {
            let case_name = &case.name;
            let case_type = case_type(context.record, context.field, case_name);
            let names = case.fields.iter().map(|field| {
                let name = &field.name;
                let binding = case_field::binding(name);
                quote!(#name: #binding)
            });
            let encoded = case.fields.iter().enumerate().map(|(index, field)| {
                let quantum = context.quantum;
                let relative = quote!((#index).saturating_mul(#quantum));
                case_field::encode(case_field::Context {
                    owner: &case_type,
                    field,
                    relative,
                    quantum: context.quantum,
                    facade: context.facade,
                    endian: context.endian,
                })
            });
            quote!(Self::#case_name(#case_type { #(#names,)* }) => { #(#encoded)* })
        })
        .collect()
}
