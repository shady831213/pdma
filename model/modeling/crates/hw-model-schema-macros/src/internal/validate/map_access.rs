use std::collections::{HashMap, hash_map::Entry};

use crate::internal::syntax::{AccessPathDecl, OrderedMapDecl};

enum PathNode {
    Group(HashMap<String, Self>),
    Leaf,
}

pub(in crate::internal) fn validate(decl: &OrderedMapDecl) -> syn::Result<()> {
    let Some(access) = &decl.access else {
        return Ok(());
    };
    let declared: HashMap<_, _> = decl
        .placements
        .iter()
        .map(|placement| (placement.symbol.to_string(), placement))
        .collect();
    let mut targets = HashMap::new();
    let mut root = HashMap::new();

    for entry in access {
        let target = entry.target.to_string();
        if !declared.contains_key(&target) {
            return Err(syn::Error::new(
                entry.target.span(),
                "access path target is not a declared root placement",
            ));
        }
        if targets.insert(target, entry.target.span()).is_some() {
            return Err(syn::Error::new(
                entry.target.span(),
                "register map placement has more than one access path",
            ));
        }
        insert_path(&mut root, entry)?;
    }

    for placement in &decl.placements {
        if !targets.contains_key(&placement.symbol.to_string()) {
            return Err(syn::Error::new(
                placement.symbol.span(),
                "register map placement is missing an access path",
            ));
        }
    }
    Ok(())
}

fn insert_path(root: &mut HashMap<String, PathNode>, entry: &AccessPathDecl) -> syn::Result<()> {
    let mut siblings = root;
    for group in &entry.groups {
        let node = match siblings.entry(group.to_string()) {
            Entry::Vacant(slot) => slot.insert(PathNode::Group(HashMap::new())),
            Entry::Occupied(slot) => slot.into_mut(),
        };
        siblings = match node {
            PathNode::Group(children) => children,
            PathNode::Leaf => {
                return Err(syn::Error::new(
                    group.span(),
                    "access path name cannot be both a leaf and a group",
                ));
            }
        };
    }
    match siblings.entry(entry.leaf.to_string()) {
        Entry::Vacant(slot) => {
            slot.insert(PathNode::Leaf);
            Ok(())
        }
        Entry::Occupied(slot) => match slot.get() {
            PathNode::Group(_) => Err(syn::Error::new(
                entry.leaf.span(),
                "access path name cannot be both a leaf and a group",
            )),
            PathNode::Leaf => Err(syn::Error::new(
                entry.leaf.span(),
                "duplicate access path sibling name",
            )),
        },
    }
}
