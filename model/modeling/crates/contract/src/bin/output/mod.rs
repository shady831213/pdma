//! Common application CLI publication, separate from schema backend rendering.
use std::{error::Error, ffi::OsStr, fs, io::Write, path::PathBuf};
/// Publishes generated text to stdout, a destination, or a comparison check.
pub fn run(
    help: &str,
    generate: impl FnOnce() -> Result<String, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let action = args.next();
    let invalid = || std::io::Error::new(std::io::ErrorKind::InvalidInput, help);
    if action.as_deref() == Some(OsStr::new("--help")) && args.next().is_none() {
        std::io::stdout().lock().write_all(help.as_bytes())?;
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
    let text = generate()?;
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
