use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::OrderedMapDecl;

use super::{ExpansionContext, Geometry};

mod codec;

pub(super) fn expand(
    decl: &OrderedMapDecl,
    context: &ExpansionContext<'_>,
    geometry: &[Geometry],
) -> TokenStream {
    let facade = context.facade;
    let marker = context.marker;
    let values_name = context.values_name;
    let raw_name = format_ident!("{}Raw", marker);
    let view_name = context.view_name;
    let view_mut_name = context.view_mut_name;
    let extent = context.extent;
    let visibility = &decl.visibility;
    let members = value_members(decl, context, geometry);
    let fields = members.fields;
    let zero_fields = members.zero_fields;
    let default_fields = members.default_fields;
    let decoded_fields = members.decoded_fields;
    let encoded_fields = members.encoded_fields;
    let codec = codec::expand(
        visibility,
        &raw_name,
        values_name,
        view_name,
        extent,
        facade,
    );
    quote! {
        #[doc = "Owned typed values for the generated register map."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility struct #values_name { #(#fields),* }
        impl #values_name {
            #[doc = "All-zero values for every declared child."] pub const ZERO: Self = Self { #(#zero_fields),* };
            #[doc = "Reset-derived defaults for every declared child."] pub const DEFAULT: Self = Self { #(#default_fields),* };
            #[doc = "Decodes one complete owned map value."] pub fn from_bytes(bytes: &[u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> { <#marker as #facade::RegisterLayout>::decode(bytes) }
            fn decode_from(bytes: &[u8]) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let actual = bytes.len();
                let bytes = bytes.get(..#extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #extent, actual })?;
                Ok(Self { #(#decoded_fields),* })
            }
            #[doc = "Encodes into one exact map-sized array."] pub fn to_bytes(self) -> ::core::result::Result<[u8; #extent], #facade::SchemaCodecError> {
                let mut bytes = [0; #extent];
                self.encode_into(&mut bytes)?;
                Ok(bytes)
            }
            #[doc = "Encodes every child and clears all map padding."] pub fn encode_into(&self, bytes: &mut [u8]) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                let actual = bytes.len();
                let output = bytes.get_mut(..#extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #extent, actual })?;
                let mut encoded = [0; #extent];
                {
                    let bytes = &mut encoded[..];
                    let value = self;
                    #(#encoded_fields)*
                }
                output.copy_from_slice(&encoded);
                Ok(())
            }
        }
        impl ::core::default::Default for #values_name {
            fn default() -> Self { Self::DEFAULT }
        }
        impl #facade::RegisterLayout for #marker {
            type Value = #values_name;
            type View<'a> = #view_name<'a>;
            type ViewMut<'a> = #view_mut_name<'a>;
            const NAME: &'static str = stringify!(#marker);
            const ALIGNMENT: usize = #marker::ALIGNMENT;
            const EXTENT: usize = #marker::EXTENT;
            const ZERO: Self::Value = #values_name::ZERO;
            const DEFAULT: Self::Value = #values_name::DEFAULT;
            fn decode(bytes: &[u8]) -> ::core::result::Result<Self::Value, #facade::SchemaCodecError> { #values_name::decode_from(bytes) }
            fn encode(bytes: &mut [u8], value: &Self::Value) -> ::core::result::Result<(), #facade::SchemaCodecError> { value.encode_into(bytes) }
            fn view(bytes: &[u8]) -> ::core::result::Result<Self::View<'_>, #facade::SchemaCodecError> { #view_name::new(bytes) }
            fn view_mut(bytes: &mut [u8]) -> ::core::result::Result<Self::ViewMut<'_>, #facade::SchemaCodecError> { #view_mut_name::new(bytes) }
            fn append_registers<I: Copy + 'static>(collection: #facade::RegisterCollection<'_, I>) {
                #marker::__append_registers(collection.id, collection.path, collection.offset, collection.declaration_index, collection.repeated, collection.output);
            }
        }
        impl #facade::RegisterMapLayout for #marker {
            fn register_definition(offset: usize) -> ::core::option::Option<#facade::RegisterDefinition<u16>> {
                let resolved = #marker::__resolved().as_ref().ok()?;
                resolved.locate(offset).map(|register| register.definition())
            }
        }
        #codec
    }
}

struct ValueMembers {
    fields: Vec<TokenStream>,
    zero_fields: Vec<TokenStream>,
    default_fields: Vec<TokenStream>,
    decoded_fields: Vec<TokenStream>,
    encoded_fields: Vec<TokenStream>,
}

fn value_members(
    decl: &OrderedMapDecl,
    context: &ExpansionContext<'_>,
    geometry: &[Geometry],
) -> ValueMembers {
    let facade = context.facade;
    let marker = context.marker;
    let mut fields = Vec::new();
    let mut zero_fields = Vec::new();
    let mut default_fields = Vec::new();
    let mut decoded_fields = Vec::new();
    let mut encoded_fields = Vec::new();
    for item in geometry {
        let field_visibility = &decl
            .placements
            .iter()
            .find(|placement| placement.symbol == item.symbol)
            .map(|placement| &placement.visibility);
        let symbol = &item.symbol;
        let definition = &item.definition;
        let placement = &item.placement;
        let count = item.count;
        if count == 1 {
            fields.push(quote!(
                #[doc = "Owned decoded child-layout value."]
                #field_visibility #symbol: <#definition as #facade::RegisterLayout>::Value
            ));
            zero_fields.push(quote!(#symbol: <#definition as #facade::RegisterLayout>::ZERO));
            default_fields.push(quote!(#symbol: <#definition as #facade::RegisterLayout>::DEFAULT));
            decoded_fields.push(quote!(#symbol: {
                let range = #marker::__placement_range(#marker::#placement, 0)?;
                let start = range.start;
                let end = range.end;
                let child = bytes.get(range).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
                <#definition as #facade::RegisterLayout>::decode(child).map_err(|error| error.rebased(#facade::SchemaCoordinate::Byte(start)))?
            }));
            encoded_fields.push(quote!({
                let range = #marker::__placement_range(#marker::#placement, 0)?;
                let start = range.start;
                let end = range.end;
                let actual = bytes.len();
                let child = bytes.get_mut(range).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
                <#definition as #facade::RegisterLayout>::encode(child, &value.#symbol).map_err(|error| error.rebased(#facade::SchemaCoordinate::Byte(start)))?;
            }));
        } else {
            fields.push(quote!(
                #[doc = "Owned decoded repeated child-layout values."]
                #field_visibility #symbol: [<#definition as #facade::RegisterLayout>::Value; #count]
            ));
            let zero_values =
                (0..count).map(|_| quote!(<#definition as #facade::RegisterLayout>::ZERO));
            zero_fields.push(quote!(#symbol: [#(#zero_values),*]));
            let default_values =
                (0..count).map(|_| quote!(<#definition as #facade::RegisterLayout>::DEFAULT));
            default_fields.push(quote!(#symbol: [#(#default_values),*]));
            let decoded = (0..count).map(|instance| {
                quote!({
                    let range = #marker::__placement_range(#marker::#placement, #instance)?;
                    let start = range.start;
                    let end = range.end;
                    let child = bytes.get(range).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual: bytes.len() })?;
                    <#definition as #facade::RegisterLayout>::decode(child).map_err(|error| error.rebased(#facade::SchemaCoordinate::Byte(start)))?
                })
            });
            decoded_fields.push(quote!(#symbol: [#(#decoded),*]));
            let bindings = (0..count)
                .map(|instance| format_ident!("{}_{}", symbol, instance))
                .collect::<Vec<_>>();
            let writes = bindings.iter().enumerate().map(|(instance, binding)| {
                quote!({
                    let range = #marker::__placement_range(#marker::#placement, #instance)?;
                    let start = range.start;
                    let end = range.end;
                    let actual = bytes.len();
                    let child = bytes.get_mut(range).ok_or(#facade::SchemaCodecError::BufferTooShort { required: end, actual })?;
                    <#definition as #facade::RegisterLayout>::encode(child, #binding).map_err(|error| error.rebased(#facade::SchemaCoordinate::Byte(start)))?;
                })
            });
            encoded_fields.push(quote!({
                let [#(#bindings),*] = &value.#symbol;
                #(#writes)*
            }));
        }
    }
    ValueMembers {
        fields,
        zero_fields,
        default_fields,
        decoded_fields,
        encoded_fields,
    }
}
