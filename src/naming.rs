//! Project-name validation and the case conversions templates need.
//!
//! A single user-supplied name has to be valid in three places at once: as a
//! Cargo package name, as a Rust identifier (the crate name), and as a Rust
//! type name (the contract struct). Validating once up front means templates
//! can use the derived forms without defensive escaping.

use anyhow::{bail, Result};

/// Rust keywords that would produce a crate name the compiler rejects.
///
/// Only the subset reachable from a valid package name matters here; a package
/// name is lowercase alphanumeric plus separators, so keywords with uppercase
/// or symbols cannot occur.
const RESERVED: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in",
    "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// Maximum length, chosen to stay well inside path limits on all platforms.
const MAX_LEN: usize = 64;

/// The three forms of a project name that templates substitute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Names {
    /// As given: the Cargo package name and the output directory name.
    pub project: String,
    /// Snake case: the crate name Rust sees, and the wasm file stem.
    pub crate_name: String,
    /// Pascal case: the contract struct name.
    pub contract: String,
}

impl Names {
    /// Validate `input` as a project name and derive the crate and contract forms.
    ///
    /// # Errors
    /// Returns an error describing the specific rule violated, so the CLI can
    /// print something a beginner can act on.
    pub fn parse(input: &str) -> Result<Self> {
        if input.is_empty() {
            bail!("project name cannot be empty");
        }
        if input.len() > MAX_LEN {
            bail!(
                "project name is {} characters; the maximum is {MAX_LEN}",
                input.len()
            );
        }

        let first = input.chars().next().expect("non-empty checked above");
        if !first.is_ascii_alphabetic() {
            bail!("project name must start with an ASCII letter, but starts with {first:?}");
        }

        if let Some(bad) = input
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
        {
            bail!(
                "project name may only contain ASCII letters, digits, '-' and '_', \
                 but contains {bad:?}"
            );
        }

        if input.ends_with('-') || input.ends_with('_') {
            bail!("project name cannot end with a separator");
        }

        let crate_name = input.replace('-', "_").to_ascii_lowercase();
        if RESERVED.contains(&crate_name.as_str()) {
            bail!("{input:?} becomes the Rust keyword {crate_name:?}, which is not a valid crate name");
        }

        Ok(Self {
            project: input.to_string(),
            contract: to_pascal_case(input),
            crate_name,
        })
    }
}

/// Convert a separator-delimited name to Pascal case: `my-token` -> `MyToken`.
fn to_pascal_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut capitalize = true;

    for c in input.chars() {
        if c == '-' || c == '_' {
            capitalize = true;
        } else if capitalize {
            out.push(c.to_ascii_uppercase());
            capitalize = false;
        } else {
            out.push(c);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_all_three_forms() {
        let names = Names::parse("my-token").unwrap();
        assert_eq!(names.project, "my-token");
        assert_eq!(names.crate_name, "my_token");
        assert_eq!(names.contract, "MyToken");
    }

    #[test]
    fn preserves_interior_capitals_in_contract_name() {
        // `to_pascal_case` only forces the first letter of each segment, so a
        // name the user already camel-cased survives intact.
        let names = Names::parse("myNFT-vault").unwrap();
        assert_eq!(names.contract, "MyNFTVault");
        assert_eq!(names.crate_name, "mynft_vault");
    }

    #[test]
    fn underscores_and_digits_are_allowed() {
        let names = Names::parse("token_v2").unwrap();
        assert_eq!(names.crate_name, "token_v2");
        assert_eq!(names.contract, "TokenV2");
    }

    #[test]
    fn rejects_empty() {
        assert!(Names::parse("").is_err());
    }

    #[test]
    fn rejects_leading_digit() {
        assert!(Names::parse("2token").is_err());
    }

    #[test]
    fn rejects_path_traversal_and_separators() {
        for bad in ["../escape", "a/b", "a b", "a.b"] {
            assert!(Names::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn rejects_trailing_separator() {
        assert!(Names::parse("token-").is_err());
        assert!(Names::parse("token_").is_err());
    }

    #[test]
    fn rejects_rust_keywords() {
        assert!(Names::parse("loop").is_err());
        // Reached through separator normalisation, not just literally.
        assert!(Names::parse("Loop").is_err());
    }

    #[test]
    fn rejects_overlong_names() {
        assert!(Names::parse(&"a".repeat(MAX_LEN + 1)).is_err());
    }
}
