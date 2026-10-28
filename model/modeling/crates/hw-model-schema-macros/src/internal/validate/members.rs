//! Checks each generated inherent namespace independently.
use std::collections::HashSet;

pub(in crate::internal) struct Members(HashSet<String>);

impl Members {
    pub(in crate::internal) fn new(reserved: &[&str]) -> Self {
        Self(reserved.iter().map(|name| (*name).to_owned()).collect())
    }

    pub(in crate::internal) fn insert(
        &mut self,
        source: &syn::Ident,
        member: &str,
    ) -> syn::Result<()> {
        let member = member.strip_prefix("r#").unwrap_or(member).to_owned();
        if self.0.insert(member.clone()) {
            Ok(())
        } else {
            Err(syn::Error::new(
                source.span(),
                format!("field generates conflicting member `{member}`"),
            ))
        }
    }
}
