use std::collections::HashSet;

use crate::internal::syntax::OrderedMapDecl;

pub(super) fn validate(decl: &OrderedMapDecl) -> syn::Result<()> {
    if decl.alignment.0 == 0 || !decl.alignment.0.is_power_of_two() {
        return Err(syn::Error::new(
            decl.alignment.1,
            "register map alignment must be a nonzero power of two",
        ));
    }
    let mut symbols = HashSet::new();
    for placement in &decl.placements {
        if !symbols.insert(placement.symbol.to_string()) {
            return Err(syn::Error::new(
                placement.symbol.span(),
                "duplicate register map placement name",
            ));
        }
        if placement.count.0 == 0 {
            return Err(syn::Error::new(
                placement.count.1,
                "register map placement count must be nonzero",
            ));
        }
        if placement.count.0 == 1 && placement.stride.is_some() {
            return Err(syn::Error::new(
                placement.stride.map_or(placement.count.1, |value| value.1),
                "stride is not valid for a singleton placement",
            ));
        }
    }
    Ok(())
}
