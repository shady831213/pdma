//! Repository-wide enforcement of native Rust module paths.

use std::error::Error;
use std::fs;
use std::path::Path;

use proc_macro2::{Delimiter, TokenStream, TokenTree};

fn redirects_module(attribute: TokenStream) -> bool {
    let mut tokens = attribute.into_iter();
    match tokens.next() {
        Some(TokenTree::Ident(name)) if name == "path" || name == "r#path" => true,
        Some(TokenTree::Ident(name)) if name == "cfg_attr" || name == "r#cfg_attr" => {
            let Some(TokenTree::Group(arguments)) = tokens.next() else {
                return false;
            };
            let mut parts = Vec::new();
            let mut part = TokenStream::new();
            for token in arguments.stream() {
                match token {
                    TokenTree::Punct(ref punctuation) if punctuation.as_char() == ',' => {
                        parts.push(std::mem::take(&mut part));
                    }
                    token => part.extend([token]),
                }
            }
            parts.push(part);
            parts.into_iter().skip(1).any(redirects_module)
        }
        _ => false,
    }
}

fn has_source_redirection(tokens: TokenStream) -> bool {
    let mut tokens = tokens.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Ident(name) if name == "include" || name == "r#include" => {
                if matches!(tokens.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '!')
                    && matches!(tokens.clone().nth(1), Some(TokenTree::Group(_)))
                {
                    return true;
                }
            }
            TokenTree::Punct(punctuation) if punctuation.as_char() == '#' => {
                if matches!(tokens.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '!') {
                    tokens.next();
                }
                if matches!(tokens.peek(), Some(TokenTree::Group(attribute))
                    if attribute.delimiter() == Delimiter::Bracket
                        && redirects_module(attribute.stream()))
                {
                    return true;
                }
            }
            TokenTree::Group(group) if has_source_redirection(group.stream()) => return true,
            _ => {}
        }
    }
    false
}

fn scan_sources(directory: &Path, violations: &mut Vec<String>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            if matches!(
                entry.file_name().to_str(),
                Some(".git" | ".omo" | ".codegraph" | "target" | "retired")
            ) {
                continue;
            }
            scan_sources(&path, violations)?;
        } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path)?;
            let tokens = source.parse::<TokenStream>().map_err(|error| {
                std::io::Error::other(format!("cannot tokenize {}: {error}", path.display()))
            })?;
            if has_source_redirection(tokens) {
                violations.push(path.display().to_string());
            }
        }
    }
    Ok(())
}

#[test]
fn active_repository_uses_native_module_paths() -> Result<(), Box<dyn Error>> {
    // Given: every Rust source in this repository except retired and generated state.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("crate is nested beneath the repository");
    let mut violations = Vec::new();

    // When: source tokens are inspected for redirection, including inactive branches.
    scan_sources(root, &mut violations)?;

    // Then: module resolution never redirects to a different filesystem path.
    violations.sort();
    assert!(
        violations.is_empty(),
        "use native mod declarations and directory layout:\n{}",
        violations.join("\n")
    );
    Ok(())
}

#[test]
fn rejects_direct_conditional_and_macro_emitted_path_attributes() {
    // Given: direct, inner, conditional, and macro-generated module redirections.
    let sources = [
        r#"#[path = "other.rs"] mod child;"#,
        r#"#[r#path = "other.rs"] mod child;"#,
        r#"#![path = "other.rs"]"#,
        r#"#[cfg_attr(any(), path = "other.rs")] mod child;"#,
        r#"#[r#cfg_attr(any(), r#path = "other.rs")] mod child;"#,
        r#"#[cfg_attr(any(), cfg_attr(all(), path = "other.rs"))] mod child;"#,
        r#"macro_rules! redirected { () => { #[path = "other.rs"] mod child; }; }"#,
    ];

    // When: token inspection encounters any of these forms.
    let rejected =
        sources.map(|source| has_source_redirection(source.parse().expect("Rust tokens")));

    // Then: no conditional or macro spelling bypasses the prohibition.
    assert!(rejected.into_iter().all(|value| value));
}

#[test]
fn rejects_source_includes_in_direct_qualified_and_nested_macro_calls() {
    // Given: source concatenation at item, expression, and macro expansion boundaries.
    let sources = [
        r#"include!("child.rs");"#,
        r#"::core::include!("child.rs");"#,
        r#"std::include! { "child.rs" }"#,
        r#"r#include!["child.rs"];"#,
        r#"mod nested { fn value() { include!(concat!("child", ".rs")); } }"#,
        r#"#[cfg(any())] mod inactive { include!("child.rs"); }"#,
        r#"macro_rules! redirected { () => { include!("child.rs"); }; }"#,
        r#"wrapper! { nested! { ::core::include!("child.rs"); } }"#,
    ];

    // When: the scanner inspects source tokens without expanding macros.
    let rejected =
        sources.map(|source| has_source_redirection(source.parse().expect("Rust tokens")));

    // Then: every source inclusion is rejected regardless of qualification or nesting.
    assert!(rejected.into_iter().all(|value| value));
}

#[test]
fn accepts_native_modules_and_mentions_inside_comments_or_strings() {
    // Given: ordinary modules and documentation mentioning an attribute as text.
    let source = r##"
        // #[path = "comment.rs"]
        // include!("comment.rs");
        #[doc = "#[path = \"documentation.rs\"]"]
        #[doc = "include!(\"documentation.rs\");"]
        mod child;
        mod inline { mod nested; }
        #[cfg_attr(test, derive(Debug))]
        struct Record;
        const EXAMPLE: &str = r#"#[path = "literal.rs"]"#;
        const SOURCE_EXAMPLE: &str = r#"include!("literal.rs");"#;
        const BYTES: &[u8] = include_bytes!("fixture.bin");
        const TEXT: &str = std::include_str!("fixture.txt");
        fn include() {}
    "##;

    // When: the scanner sees comments and string literals alongside real attributes.
    let rejected = has_source_redirection(source.parse().expect("Rust tokens"));

    // Then: native modules and data inclusion remain accepted.
    assert!(!rejected);
}
