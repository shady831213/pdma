//! Exports DMA register metadata for the native Veryl design.

/// Schema-backed assembly of the DMA RTL layout packages.
pub mod rtl_layout;

use std::{error::Error, ffi::OsStr, fs, io::Write, path::PathBuf};

const HELP: &str = "Usage: pdma-rtl-layout [--write FILE | --check FILE | --help]\n\
    Without arguments, writes the generated Veryl package to stdout.\n";

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let action = args.next();
    let invalid = || std::io::Error::new(std::io::ErrorKind::InvalidInput, HELP);
    if action.as_deref() == Some(OsStr::new("--help")) && args.next().is_none() {
        std::io::stdout().lock().write_all(HELP.as_bytes())?;
        return Ok(());
    }
    let destination = match action.as_deref() {
        None => None,
        Some(value) if value == OsStr::new("--write") || value == OsStr::new("--check") => {
            Some(PathBuf::from(args.next().ok_or_else(invalid)?))
        }
        Some(_) => return Err(invalid().into()),
    };
    if args.next().is_some() {
        return Err(invalid().into());
    }
    let text = rtl_layout::generate()?;
    if let Some(path) = destination {
        if action.as_deref() == Some(OsStr::new("--check")) {
            if fs::read(&path)? != text.as_bytes() {
                return Err(std::io::Error::other(format!(
                    "{} is stale; regenerate with --write",
                    path.display()
                ))
                .into());
            }
        } else {
            fs::write(path, text)?;
        }
    } else {
        std::io::stdout().lock().write_all(text.as_bytes())?;
    }
    Ok(())
}
