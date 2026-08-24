use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{FieldDecl, FieldShape, RecordDecl, RecordPlacementDecl, ScalarType};

pub(super) fn expand(decl: &RecordDecl, facade: &TokenStream) -> TokenStream {
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let values_name = format_ident!("{}Values", marker);
    let view_name = format_ident!("{}View", marker);
    let view_mut_name = format_ident!("{}ViewMut", marker);
    let size = decl.size.0;
    let alignment = decl.alignment.0;
    let ValueMembers {
        fields,
        zero_fields,
        decoded_fields,
        encoded_fields,
    } = value_members(decl, facade);

    quote! {
        #[doc = "Owned typed values for the generated record."]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        #visibility struct #values_name { #(#fields),* }
        impl #values_name {
            #[doc = "All-zero values for every declared field and nested record."]
            pub const ZERO: Self = Self { #(#zero_fields),* };
            #[doc = "Decodes one complete owned record value."]
            pub fn from_bytes(bytes: &[u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> { Self::decode_from(bytes) }
            #[doc = "Decodes one complete owned record value."]
            pub fn decode_from(bytes: &[u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let actual = bytes.len();
                let bytes = bytes.get(..#size).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #size, actual })?;
                Ok(Self { #(#decoded_fields),* })
            }
            #[doc = "Encodes into one exact record-sized array."]
            pub fn to_bytes(self) -> ::core::result::Result<[u8; #size], #facade::SchemaCodecError> { self.encoded() }
            #[doc = "Encodes every field and clears all record padding."]
            pub fn encode_into(&self, bytes: &mut [u8]) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                let actual = bytes.len();
                let destination = bytes.get_mut(..#size).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #size, actual })?;
                let encoded = self.encoded()?;
                destination.copy_from_slice(&encoded);
                Ok(())
            }
            fn encoded(&self) -> ::core::result::Result<[u8; #size], #facade::SchemaCodecError> {
                let value = self;
                let mut encoded = [0; #size];
                #(#encoded_fields)*
                Ok(encoded)
            }
        }
        impl #facade::RecordLayout for #marker {
            type Value = #values_name;
            type View<'a> = #view_name<'a>;
            type ViewMut<'a> = #view_mut_name<'a>;
            const NAME: &'static str = stringify!(#marker);
            const ALIGNMENT: usize = #alignment;
            const EXTENT: usize = #size;
            const ZERO: Self::Value = #values_name::ZERO;
            fn decode(bytes: &[u8]) -> ::core::result::Result<Self::Value, #facade::SchemaCodecError> { #values_name::decode_from(bytes) }
            fn encode(bytes: &mut [u8], value: &Self::Value) -> ::core::result::Result<(), #facade::SchemaCodecError> { value.encode_into(bytes) }
            fn view(bytes: &[u8]) -> ::core::result::Result<Self::View<'_>, #facade::SchemaCodecError> { #view_name::new(bytes) }
            fn view_mut(bytes: &mut [u8]) -> ::core::result::Result<Self::ViewMut<'_>, #facade::SchemaCodecError> { #view_mut_name::new(bytes) }
        }
    }
}

#[derive(Default)]
struct ValueMembers {
    fields: Vec<TokenStream>,
    zero_fields: Vec<TokenStream>,
    decoded_fields: Vec<TokenStream>,
    encoded_fields: Vec<TokenStream>,
}

#[derive(Clone, Copy)]
struct ExpansionContext<'a> {
    visibility: &'a syn::Visibility,
    facade: &'a TokenStream,
    size: usize,
}

fn value_members(decl: &RecordDecl, facade: &TokenStream) -> ValueMembers {
    let mut output = ValueMembers::default();
    let context = ExpansionContext {
        visibility: &decl.visibility,
        facade,
        size: decl.size.0,
    };
    for field in &decl.fields {
        push_field(&mut output, field, context);
    }
    for record in &decl.records {
        push_record(&mut output, record, context);
    }
    output
}

fn push_field(output: &mut ValueMembers, field: &FieldDecl, context: ExpansionContext<'_>) {
    let ExpansionContext {
        visibility,
        facade,
        size,
    } = context;
    let name = &field.name;
    let base = field.offset.0;
    match &field.shape {
        FieldShape::Scalar { scalar, .. } => {
            let ty = scalar_tokens(*scalar);
            let endian = &field.endian;
            let end = base + scalar.width();
            output
                .fields
                .push(quote!(#[doc = "Owned decoded scalar field value."] #visibility #name: #ty));
            output.zero_fields.push(quote!(#name: 0));
            output.decoded_fields.push(quote!(#name: {
                let field = bytes.get(#base..#end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #end, actual: bytes.len() })?;
                #facade::read_scalar(field, #facade::Endian::#endian)?
            }));
            output.encoded_fields.push(quote!({
                let field = encoded.get_mut(#base..#end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #end, actual: #size })?;
                #facade::write_scalar(field, #facade::Endian::#endian, value.#name)?;
            }));
        }
        FieldShape::Reference { definition, .. } => {
            let ty = quote!(<#definition as #facade::FieldSchema>::Scalar);
            output.fields.push(quote!(#[doc = "Owned decoded reusable scalar field value."] #visibility #name: #ty));
            output
                .zero_fields
                .push(quote!(#name: <#definition as #facade::FieldSchema>::ZERO));
            output.decoded_fields.push(quote!(#name: {
                let end = #base.checked_add(#definition::DEFINITION.width()).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
                let field = bytes.get(#base..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
                <#definition as #facade::FieldSchema>::read(field)?
            }));
            output.encoded_fields.push(quote!({
                let end = #base.checked_add(#definition::DEFINITION.width()).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: #size })?;
                let field = encoded.get_mut(#base..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: #size })?;
                <#definition as #facade::FieldSchema>::write(field, value.#name)?;
            }));
        }
        FieldShape::Array {
            scalar,
            count,
            stride,
            ..
        } => {
            push_array(output, field, *scalar, count.0, stride.0, context);
        }
    }
}

fn push_array(
    output: &mut ValueMembers,
    field: &FieldDecl,
    scalar: ScalarType,
    count: usize,
    stride: usize,
    context: ExpansionContext<'_>,
) {
    let ExpansionContext {
        visibility,
        facade,
        size,
    } = context;
    let name = &field.name;
    let base = field.offset.0;
    let endian = &field.endian;
    let ty = scalar_tokens(scalar);
    output.fields.push(quote!(#[doc = "Owned decoded scalar-array field values."] #visibility #name: [#ty; #count]));
    output.zero_fields.push(quote!(#name: [0; #count]));
    let decoded = (0..count).map(|index| {
        let offset = base + index * stride;
        let end = offset + scalar.width();
        quote!({
            let field = bytes.get(#offset..#end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #end, actual: bytes.len() })?;
            #facade::read_scalar(field, #facade::Endian::#endian)?
        })
    });
    output.decoded_fields.push(quote!(#name: [#(#decoded),*]));
    for index in 0..count {
        let offset = base + index * stride;
        let end = offset + scalar.width();
        output.encoded_fields.push(quote!({
            let field = encoded.get_mut(#offset..#end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #end, actual: #size })?;
            #facade::write_scalar(field, #facade::Endian::#endian, value.#name[#index])?;
        }));
    }
}

fn push_record(
    output: &mut ValueMembers,
    record: &RecordPlacementDecl,
    context: ExpansionContext<'_>,
) {
    let ExpansionContext {
        visibility,
        facade,
        size,
    } = context;
    let name = &record.name;
    let definition = &record.definition;
    let offset = record.offset.0;
    output.fields.push(quote!(#[doc = "Owned decoded nested-record value."] #visibility #name: <#definition as #facade::RecordLayout>::Value));
    output
        .zero_fields
        .push(quote!(#name: <#definition as #facade::RecordLayout>::ZERO));
    output.decoded_fields.push(quote!(#name: {
        let end = #offset.checked_add(<#definition as #facade::RecordLayout>::EXTENT).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: bytes.len() })?;
        let record = bytes.get(#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
        <#definition as #facade::RecordLayout>::decode(record)?
    }));
    output.encoded_fields.push(quote!({
        let end = #offset.checked_add(<#definition as #facade::RecordLayout>::EXTENT).ok_or(#facade::SchemaCodecError::BufferTooShort { required: usize::MAX, actual: #size })?;
        let record = encoded.get_mut(#offset..end).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: #size })?;
        <#definition as #facade::RecordLayout>::encode(record, &value.#name)?;
    }));
}

fn scalar_tokens(scalar: ScalarType) -> TokenStream {
    match scalar {
        ScalarType::U8 => quote!(u8),
        ScalarType::U16 => quote!(u16),
        ScalarType::U32 => quote!(u32),
        ScalarType::U64 => quote!(u64),
    }
}
