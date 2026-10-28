use std::collections::HashSet;

use crate::internal::syntax::OrderedMapDecl;

pub(super) fn validate(decl: &OrderedMapDecl) -> syn::Result<()> {
    crate::internal::validate::map_access::validate(decl)?;
    if decl.alignment.0 == 0 || !decl.alignment.0.is_power_of_two() {
        return Err(syn::Error::new(
            decl.alignment.1,
            "register map alignment must be a nonzero power of two",
        ));
    }
    let mut symbols = HashSet::new();
    let mut parameters = HashSet::new();
    for parameter in &decl.parameters {
        if !parameters.insert(parameter.to_string()) {
            return Err(syn::Error::new(parameter.span(), "duplicate map parameter"));
        }
    }
    for placement in &decl.placements {
        if let Some(parameter) = &placement.count_parameter
            && !parameters.contains(&parameter.to_string())
        {
            return Err(syn::Error::new(
                parameter.span(),
                "bank count must name a declared map parameter",
            ));
        }
        for argument in placement.arguments.iter().flatten() {
            if let syn::Expr::Path(path) = argument
                && !path
                    .path
                    .get_ident()
                    .is_some_and(|name| parameters.contains(&name.to_string()))
            {
                return Err(syn::Error::new_spanned(
                    argument,
                    "child argument must name a declared map parameter",
                ));
            }
        }
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
    }
    Ok(())
}
