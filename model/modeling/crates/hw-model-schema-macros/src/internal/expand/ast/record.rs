use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;

pub(in crate::internal::expand) fn expand(
    decl: &ModernRecordDecl,
    facade: &TokenStream,
) -> TokenStream {
    let marker = &decl.name;
    let endian = &decl.endian;
    let alignment = decl.alignment.map_or_else(
        || quote!(::core::option::Option::None),
        |(alignment, _)| quote!(::core::option::Option::Some(#alignment)),
    );
    let fields = decl.fields.iter().map(|field| {
        let name = field.name.to_string();
        let offset = format_ident!("{}_OFFSET", field.name.unraw().to_string().to_ascii_uppercase());
        let documentation = field.documentation.metadata(facade);
        let (ty, extent) = match &field.ty {
            ModernTypeDecl::Scalar { scalar, constant } => {
                let kind = crate::internal::expand::scalar::kind(*scalar);
                let width = scalar.width();
                let constant = constant.as_ref().map_or_else(|| quote!(::core::option::Option::None), |path| {
                    {
                        let value = crate::internal::expand::scalar::record_size_path(path, facade);
                        let value = crate::internal::expand::scalar::from_usize(*scalar, &value);
                        quote!(::core::option::Option::Some(#facade::ast::Constant { symbol: ::core::stringify!(#path), value: ::core::convert::Into::<u64>::into(#value) }))
                    }
                });
                (quote!(#facade::ast::FieldType::Scalar { definition: #facade::FieldDefinition::new(#facade::ScalarKind::#kind).with_endian(#facade::Endian::#endian), constant: #constant }), quote!(#width))
            }
            ModernTypeDecl::Enum(enumeration) => {
                let kind = crate::internal::expand::scalar::kind(enumeration.storage);
                let width = enumeration.storage.width();
                let metadata = crate::internal::expand::enumeration::metadata(enumeration, facade);
                (quote!(#facade::ast::FieldType::Enum(#facade::FieldDefinition::new(#facade::ScalarKind::#kind).with_endian(#facade::Endian::#endian).with_enum(#metadata))), quote!(#width))
            }
            ModernTypeDecl::Bits(bits) => {
                let kind = crate::internal::expand::scalar::kind(bits.storage);
                let width = bits.storage.width();
                let fields = bits.fields.iter().enumerate().map(|(index, bit)| {
                    let name = bit.name.to_string();
                    let lsb = bit.start.0;
                    let width = crate::internal::expand::bits::width(bit);
                    let documentation = bit.documentation.metadata(facade);
                    let enumeration = bit.enumeration.as_ref().map(|decl| {
                        let metadata = crate::internal::expand::enumeration::metadata(decl, facade);
                        quote!(.with_enum(#metadata))
                    });
                    quote!(#facade::BitsFieldSpec::place(
                        match #facade::inferred_field_id(#index) { ::core::option::Option::Some(id) => id, ::core::option::Option::None => 0 },
                        #name, #lsb, #facade::BitsFieldDefinition::new(#width)#enumeration,
                    ).with_declaration_index(#index).with_documentation(#documentation))
                });
                (quote!({
                    const FIELDS: &[#facade::BitsFieldSpec] = &[#(#fields),*];
                    #facade::ast::FieldType::Bits(#facade::ast::BitsDefinition::new(#facade::ScalarKind::#kind, FIELDS))
                }), quote!(#width))
            }
            ModernTypeDecl::Codec(path) => (
                quote!(#facade::ast::FieldType::Reference(#facade::ast::Reference::new(#name, #facade::ast::ReferenceKind::Field, <#path as #facade::SchemaLayout>::schema_definition))),
                quote!(<#path as #facade::SchemaLayout>::EXTENT),
            ),
            ModernTypeDecl::BitsReference(path) => (
                quote!(#facade::ast::FieldType::Reference(#facade::ast::Reference::new(#name, #facade::ast::ReferenceKind::Field, <#path as #facade::BitsLayout>::schema_definition))),
                quote!(<#path as #facade::BitsLayout>::EXTENT),
            ),
            ModernTypeDecl::TaggedUnion(_) => {
                let ty = crate::internal::expand::scalar::type_name(marker, &field.name, "Values");
                (quote!(#facade::ast::FieldType::Union(::std::boxed::Box::new(<#ty as #facade::ast::DescribeUnion>::schema_union()))), quote!(<#ty as #facade::ast::DescribeUnion>::schema_union().storage().size()))
            }
        };
        quote!(#facade::ast::Field::new(#name, Self::#offset, #extent, #facade::Endian::#endian, #ty).with_documentation(#documentation))
    });
    quote! {
        #facade::__register_schema!(<#marker as #facade::ast::DescribeSchema>::schema_definition);
        impl #facade::ast::DescribeSchema for #marker {
            fn schema_definition() -> #facade::ast::Definition {
                #facade::ast::Definition::record::<Self>(Self::SPEC, ::std::vec![#(#fields),*]).with_record_declaration(#alignment, #facade::Endian::#endian)
            }
        }
    }
}
