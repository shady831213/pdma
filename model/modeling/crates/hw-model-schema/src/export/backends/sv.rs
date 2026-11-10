//! Private `SystemVerilog` target syntax and namespace bookkeeping.
mod identifier;
use crate::{export::EntrySymbols, traversal::NodeId};
use std::{
    collections::HashMap,
    io::{self, Write},
};

pub(super) fn valid_name(name: &str) -> io::Result<()> {
    if identifier::identifier(name) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid SystemVerilog identifier {name:?}"),
        ))
    }
}
pub(super) fn comment(text: &str, w: &mut dyn Write) -> io::Result<()> {
    for line in text.lines() {
        writeln!(
            w,
            "    // {}",
            line.replace(['\r', '\u{2028}', '\u{2029}'], " ")
        )?;
    }
    Ok(())
}
#[derive(Debug, Default)]
pub(super) struct Names(HashMap<NodeId, String>);
impl Names {
    pub(super) fn set(&mut self, id: NodeId, name: &str) {
        self.0.insert(id, name.into());
    }
    pub(super) fn get(&self, id: &NodeId, fallback: &str) -> String {
        self.0.get(id).cloned().unwrap_or_else(|| fallback.into())
    }
    pub(super) fn check(
        &self,
        id: &NodeId,
        fallback: &str,
        symbols: &mut EntrySymbols,
    ) -> io::Result<()> {
        let name = self.get(id, fallback);
        valid_name(&name)?;
        symbols
            .declare(&name, id.source_path().unwrap_or(fallback))
            .map_err(io::Error::other)
    }
}
/// A target namespace retains other entries but permits retrying one entry's output.
#[derive(Debug, Default)]
pub(super) struct Symbols(HashMap<String, NodeId>);
impl Symbols {
    /// A type may be referenced repeatedly, but cannot share a local declaration's name.
    #[cfg(feature = "export-uvm-regs")]
    pub(super) fn reference_type(&mut self, name: &str, target: &NodeId) -> io::Result<()> {
        match self.0.get(name) {
            Some(previous) if previous == target => Ok(()),
            Some(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("SystemVerilog type {name} is shadowed by a local name; select an alias"),
            )),
            None => self.declare(name, target),
        }
    }
    #[cfg(feature = "export-sv")]
    pub(super) fn begin(&mut self, owner: &NodeId) {
        self.0.retain(|_, v| v != owner);
    }
    pub(super) fn declare(&mut self, name: &str, owner: &NodeId) -> io::Result<()> {
        valid_name(name)?;
        if self.0.contains_key(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("duplicate SystemVerilog symbol {name}"),
            ));
        }
        self.0.insert(name.into(), owner.clone());
        Ok(())
    }
}
