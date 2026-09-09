use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{RegisterDefinitionDecl, RegisterFieldsDecl};

pub(super) struct FieldsExpansion {
    pub(super) id_declaration: TokenStream,
    pub(super) id_type: TokenStream,
    pub(super) specs: TokenStream,
    pub(super) identities: TokenStream,
    pub(super) getters: Vec<TokenStream>,
    pub(super) setters: Vec<TokenStream>,
    pub(super) value_methods: Vec<TokenStream>,
    pub(super) assertions: TokenStream,
}

pub(super) fn expand(
    decl: &RegisterDefinitionDecl,
    facade: &TokenStream,
) -> syn::Result<FieldsExpansion> {
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let id_name = format_ident!("{}FieldId", marker);
    match &decl.fields {
        RegisterFieldsDecl::Inline(declared) => {
            expand_inline(declared, decl.width.0, visibility, marker, &id_name, facade)
        }
        RegisterFieldsDecl::Reference(path) => Ok(expand_reference(decl, path, &id_name, facade)),
    }
}

fn expand_reference(
    decl: &RegisterDefinitionDecl,
    path: &syn::Path,
    id_name: &syn::Ident,
    facade: &TokenStream,
) -> FieldsExpansion {
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let width_bits = decl.width.0 * 8;
    let writable_mask = decl.mask.0;
    let getters = vec![quote! {
        #[doc = "Decodes the reusable typed bits value."]
        pub fn fields(&self) -> ::core::result::Result<<#path as #facade::BitsLayout>::Value, #facade::RegisterCodecError> {
            let raw = #marker::SPEC.view(self.bytes)?.value()?;
            ::core::result::Result::Ok(<#path as #facade::BitsLayout>::decode_register(raw))
        }
    }];
    let setters = vec![quote! {
        #[doc = "Encodes the reusable typed bits value while preserving undeclared bits."]
        pub fn set_fields(&mut self, value: <#path as #facade::BitsLayout>::Value) -> ::core::result::Result<(), #facade::RegisterCodecError> {
            let encoded = <#path as #facade::BitsLayout>::encode_register(&value)
                .map_err(|error| error.rebased(self.base))?;
            let declared_mask = <#path as #facade::BitsLayout>::DECLARED_MASK;
            let mut view = #marker::SPEC.view_mut(self.bytes)?;
            let current = view.value()?;
            view.set_value((current & !declared_mask) | encoded)
        }
    }];
    let narrowed = scalar_from_u64(
        &format_ident!("u{}", decl.width.0 * 8),
        decl.width.0,
        &quote!(combined),
    );
    let value_methods = vec![quote! {
        #[doc = "Decodes the reusable typed bits value."]
        pub fn fields(self) -> <#path as #facade::BitsLayout>::Value {
            <#path as #facade::BitsLayout>::decode_register(u64::from(self.0))
        }
        #[doc = "Replaces declared fields while preserving every other register bit."]
        pub fn with_fields(self, value: <#path as #facade::BitsLayout>::Value) -> ::core::result::Result<Self, #facade::RegisterCodecError> {
            let encoded = <#path as #facade::BitsLayout>::encode_register(&value)?;
            let declared_mask = <#path as #facade::BitsLayout>::DECLARED_MASK;
            let combined = (u64::from(self.0) & !declared_mask) | encoded;
            ::core::result::Result::Ok(Self(#narrowed))
        }
    }];
    FieldsExpansion {
        id_declaration: quote! {
            #[doc = "Stable field identity reused from the referenced bits schema."]
            #visibility type #id_name = <#path as #facade::BitsLayout>::FieldId;
        },
        id_type: quote!(u16),
        specs: quote!(<#path as #facade::BitsLayout>::FIELDS),
        identities: TokenStream::new(),
        getters,
        setters,
        value_methods,
        assertions: quote! {
            const _: () = assert!(#path::BIT_WIDTH <= #width_bits, "bits schema exceeds register width");
            const _: () = assert!(#writable_mask & !#path::DECLARED_MASK == 0, "writable mask contains undeclared bits");
        },
    }
}

fn expand_inline(
    declared: &[crate::internal::syntax::BitFieldDecl],
    register_width: usize,
    visibility: &syn::Visibility,
    marker: &syn::Ident,
    id_name: &syn::Ident,
    facade: &TokenStream,
) -> syn::Result<FieldsExpansion> {
    let mut variants = Vec::new();
    let mut fields = Vec::new();
    let mut identities = Vec::new();
    let mut getters = Vec::new();
    let mut setters = Vec::new();
    let mut value_methods = Vec::new();
    for (index, field) in declared.iter().enumerate() {
        let method = &field.name;
        let name = method.to_string();
        let documentation = field.documentation.metadata(facade);
        let rust_docs = field.documentation.rust_docs();
        let identity = format_ident!("{}_IDENTITY", name.to_ascii_uppercase());
        let id = u16::try_from(index.saturating_add(1))
            .map_err(|_| syn::Error::new(method.span(), "register field ID exceeds u16"))?;
        let lsb = field.start.0;
        let bit_width = crate::internal::expand::bits::width(field);
        identities.push(quote! {
            #[doc = "Stable identity for one declared register field."]
            pub const #identity: #facade::SchemaMemberIdentity =
                #facade::SchemaMemberIdentity::new(stringify!(#marker), #name);
        });
        let semantic_type = crate::internal::expand::bits::semantic_type(field);
        variants.push(super::super::variant(&name));
        fields.push(quote!(#facade::BitsFieldSpec::place(#id, #name, #lsb, #facade::BitsFieldDefinition::new(#bit_width)).with_declaration_index(#index).with_documentation(#documentation)));
        let raw = quote!(raw);
        let decoded = crate::internal::expand::bits::decode_local(field, &raw);
        getters.push(quote!(#rust_docs #[doc = "Reads the named register field."] pub fn #method(&self) -> ::core::result::Result<#semantic_type, #facade::RegisterCodecError> { let raw = #marker::SPEC.view(self.bytes)?.field_by_name(#name)?; Ok(#decoded) }));
        let setter = format_ident!("set_{}", method);
        let value = quote!(value);
        let encoded = crate::internal::expand::bits::encode_local(&value);
        setters.push(quote!(#rust_docs #[doc = "Writes the named register field."] pub fn #setter(&mut self, value: #semantic_type) -> ::core::result::Result<(), #facade::RegisterCodecError> { #marker::SPEC.view_mut(self.bytes)?.set_field_by_name(#name, #encoded).map_err(|error| error.rebased(self.base)) }));
        let full = quote!(u64::from(self.0));
        let decoded_value = crate::internal::expand::bits::decode(field, &full);
        let with_method = format_ident!("with_{}", method);
        let maximum = crate::internal::validate::bits::bit_mask(bit_width);
        let field_mask = maximum << lsb;
        let narrowed = scalar_from_u64(
            &format_ident!("u{}", register_width * 8),
            register_width,
            &quote!(combined),
        );
        value_methods.push(quote! {
            #rust_docs
            #[doc = "Reads the named register field from the owned value."]
            pub fn #method(self) -> #semantic_type { #decoded_value }
            #rust_docs
            #[doc = "Replaces the named field while preserving every other register bit."]
            pub fn #with_method(self, value: #semantic_type) -> ::core::result::Result<Self, #facade::RegisterCodecError> {
                let observed = u64::from(value);
                if observed > #maximum {
                    return ::core::result::Result::Err(#facade::RegisterCodecError::FieldValueOverflow { location: #marker::#identity.at_bit(#lsb), bit_width: #bit_width, value: observed });
                }
                let combined = (u64::from(self.0) & !#field_mask) | (observed << #lsb);
                ::core::result::Result::Ok(Self(#narrowed))
            }
        });
    }
    let documented_variants = variants
        .iter()
        .map(|item| quote!(#[doc = "Stable generated field ID."] #item));
    Ok(FieldsExpansion {
        id_declaration: quote! {
            #[doc = "Stable sibling-local register-field identities."]
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            #visibility enum #id_name { #(#documented_variants),* }
        },
        id_type: quote!(u16),
        specs: quote!(&[#(#fields),*]),
        identities: quote!(#(#identities)*),
        getters,
        setters,
        value_methods,
        assertions: TokenStream::new(),
    })
}

pub(super) fn scalar_from_u64(
    scalar: &syn::Ident,
    width: usize,
    value: &TokenStream,
) -> TokenStream {
    let bytes = (0..width).map(syn::Index::from);
    quote!({
        let bytes = (#value).to_le_bytes();
        #scalar::from_le_bytes([#(bytes[#bytes]),*])
    })
}
