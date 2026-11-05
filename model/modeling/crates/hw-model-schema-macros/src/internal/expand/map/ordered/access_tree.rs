use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::{
    expand::register::variant,
    syntax::{AccessPathDecl, OrderedMapDecl},
};

use super::{ExpansionContext, Geometry, accessors};

pub(super) struct Expansion {
    pub(super) items: TokenStream,
    pub(super) root_immutable: TokenStream,
    pub(super) root_mutable: TokenStream,
}

struct Group {
    name: syn::Ident,
    path: Vec<syn::Ident>,
    groups: Vec<Self>,
    leaves: Vec<Leaf>,
}

struct Leaf {
    name: syn::Ident,
    target: syn::Ident,
}

struct Emitter<'a, 'context> {
    decl: &'a OrderedMapDecl,
    context: &'a ExpansionContext<'context>,
    geometry: &'a [Geometry],
    output: Vec<TokenStream>,
    types: crate::internal::validate::members::Members,
}

pub(super) fn expand(
    decl: &OrderedMapDecl,
    context: &ExpansionContext<'_>,
    geometry: &[Geometry],
) -> syn::Result<Expansion> {
    let mut roots = Vec::new();
    let entries = decl.access.iter().flatten();
    for entry in entries.clone().filter(|entry| !entry.groups.is_empty()) {
        insert(&mut roots, entry, &[])?;
    }
    let mut emitter = Emitter {
        decl,
        context,
        geometry,
        output: Vec::new(),
        types: crate::internal::validate::members::Members::new(&[]),
    };
    for group in &roots {
        emitter.emit_group(group)?;
    }
    let group_methods = group_methods(&roots, context);
    let root_leaves = entries
        .filter(|entry| entry.groups.is_empty())
        .collect::<Vec<_>>();
    let leaf_methods = leaf_methods(&root_leaves, geometry, context)?;
    let group_immutable = group_methods.0;
    let group_mutable = group_methods.1;
    let leaf_immutable = leaf_methods.0;
    let leaf_mutable = leaf_methods.1;
    let items = emitter.output;
    Ok(Expansion {
        items: quote!(#(#items)*),
        root_immutable: quote!(#group_immutable #leaf_immutable),
        root_mutable: quote!(#group_mutable #leaf_mutable),
    })
}

fn leaf_methods(
    entries: &[&AccessPathDecl],
    geometry: &[Geometry],
    context: &ExpansionContext<'_>,
) -> syn::Result<(TokenStream, TokenStream)> {
    let mut immutable = Vec::new();
    let mut mutable = Vec::new();
    for &entry in entries {
        let item = find_geometry(&entry.target, geometry)?;
        if entry.leaf == item.symbol {
            continue;
        }
        let pair = accessors::leaf_getters(&entry.leaf, item, context);
        immutable.push(pair.immutable);
        mutable.push(pair.mutable);
    }
    Ok((quote!(#(#immutable)*), quote!(#(#mutable)*)))
}

fn insert(
    groups: &mut Vec<Group>,
    entry: &AccessPathDecl,
    parent: &[syn::Ident],
) -> syn::Result<()> {
    let Some((head, tail)) = entry.groups.split_first() else {
        return Ok(());
    };
    let index = groups.iter().position(|group| group.name == *head);
    let group = if let Some(index) = index {
        groups
            .get_mut(index)
            .ok_or_else(|| syn::Error::new(head.span(), "missing access group"))?
    } else {
        let mut path = parent.to_vec();
        path.push(head.clone());
        groups.push(Group {
            name: head.clone(),
            path,
            groups: Vec::new(),
            leaves: Vec::new(),
        });
        groups
            .last_mut()
            .ok_or_else(|| syn::Error::new(head.span(), "missing access group"))?
    };
    if tail.is_empty() {
        group.leaves.push(Leaf {
            name: entry.leaf.clone(),
            target: entry.target.clone(),
        });
    } else {
        let path = group.path.clone();
        let nested = AccessPathDecl {
            groups: tail.to_vec(),
            leaf: entry.leaf.clone(),
            target: entry.target.clone(),
        };
        insert(&mut group.groups, &nested, &path)?;
    }
    Ok(())
}

impl Emitter<'_, '_> {
    fn emit_group(&mut self, group: &Group) -> syn::Result<()> {
        let visibility = &self.decl.visibility;
        let (view, view_mut) = type_names(self.context.marker, &group.path);
        self.types.insert(&group.name, &view.to_string())?;
        self.types.insert(&group.name, &view_mut.to_string())?;
        let nested = group_methods(&group.groups, self.context);
        let nested_immutable = nested.0;
        let nested_mutable = nested.1;
        let mut immutable_leaves = Vec::new();
        let mut mutable_leaves = Vec::new();
        for leaf in &group.leaves {
            let item = find_geometry(&leaf.target, self.geometry)?;
            let pair = accessors::leaf_getters(&leaf.name, item, self.context);
            immutable_leaves.push(pair.immutable);
            mutable_leaves.push(pair.mutable);
        }
        self.output.push(quote! {
            #[doc = "Immutable generated register access group over the full map image."]
            #[derive(Debug)]
            #visibility struct #view<'a> { bytes: &'a [u8] }
            impl<'a> #view<'a> {
                #nested_immutable
                #(#immutable_leaves)*
            }
            #[doc = "Mutable generated register access group over the full map image."]
            #[derive(Debug)]
            #visibility struct #view_mut<'a> { bytes: &'a mut [u8] }
            impl<'a> #view_mut<'a> {
                #nested_immutable
                #nested_mutable
                #(#immutable_leaves)*
                #(#mutable_leaves)*
            }
        });
        for child in &group.groups {
            self.emit_group(child)?;
        }
        Ok(())
    }
}

fn find_geometry<'a>(target: &syn::Ident, geometry: &'a [Geometry]) -> syn::Result<&'a Geometry> {
    geometry
        .iter()
        .find(|item| item.symbol == *target)
        .ok_or_else(|| syn::Error::new(target.span(), "unknown access path target"))
}

fn group_methods(groups: &[Group], context: &ExpansionContext<'_>) -> (TokenStream, TokenStream) {
    let facade = context.facade;
    let extent = context.extent;
    let mut immutable = Vec::new();
    let mut mutable = Vec::new();
    for group in groups {
        let method = &group.name;
        let mutable_method = format_ident!("{}_mut", method);
        let (view, view_mut) = type_names(context.marker, &group.path);
        immutable.push(quote! {
            #[doc = "Returns an immutable view of one declared access group."]
            pub fn #method(&self) -> ::core::result::Result<#view<'_>, #facade::SchemaCodecError> {
                let actual = self.bytes.len();
                let bytes = self.bytes.get(..#extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #extent, actual })?;
                ::core::result::Result::Ok(#view { bytes })
            }
        });
        mutable.push(quote! {
            #[doc = "Returns a mutable view of one declared access group."]
            pub fn #mutable_method(&mut self) -> ::core::result::Result<#view_mut<'_>, #facade::SchemaCodecError> {
                let actual = self.bytes.len();
                let bytes = self.bytes.get_mut(..#extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: #extent, actual })?;
                ::core::result::Result::Ok(#view_mut { bytes })
            }
        });
    }
    (quote!(#(#immutable)*), quote!(#(#mutable)*))
}

fn type_names(marker: &syn::Ident, path: &[syn::Ident]) -> (syn::Ident, syn::Ident) {
    let suffix = path
        .iter()
        .map(|segment| variant(segment).to_string())
        .collect::<String>();
    (
        format_ident!("{marker}{suffix}View"),
        format_ident!("{marker}{suffix}ViewMut"),
    )
}
