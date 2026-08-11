//! Token substitution and file writing.
//!
//! # Why `{% token %}` and not `{{ token }}`
//!
//! Generated projects ship a GitHub Actions workflow, and Actions expressions
//! are spelled `${{ matrix.os }}`. A `{{ }}` delimiter would collide with them,
//! so templates would need escaping precisely in the files most likely to be
//! copied from upstream examples. `{%` appears in neither GitHub Actions
//! expressions, TOML, nor Rust, so templates stay literal.
//!
//! Substitution is deliberately not a template engine: there are no conditionals
//! or loops. A template that needs branching should be a separate template, which
//! keeps every template readable as the file it will produce.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context as _, Result};

use crate::manifest::Template;
use crate::naming::Names;
use crate::toolchain;

/// The values available to templates as `{% token %}`.
///
/// Toolchain tokens come from [`crate::toolchain`], so a template never hard-codes
/// a version and a bump reaches every template at once.
#[derive(Debug, Clone)]
pub struct Context {
    tokens: BTreeMap<&'static str, String>,
}

impl Context {
    /// Build the substitution context for a project.
    pub fn new(names: &Names) -> Self {
        let mut tokens = BTreeMap::new();

        tokens.insert("project_name", names.project.clone());
        tokens.insert("crate_name", names.crate_name.clone());
        tokens.insert("contract_name", names.contract.clone());

        tokens.insert("sdk_version", toolchain::SOROBAN_SDK_VERSION.to_string());
        tokens.insert("rust_target", toolchain::RUST_TARGET.to_string());
        tokens.insert("msrv", toolchain::MSRV.to_string());
        tokens.insert("rust_edition", toolchain::RUST_EDITION.to_string());
        tokens.insert(
            "stellar_cli_version",
            toolchain::STELLAR_CLI_VERSION.to_string(),
        );
        tokens.insert("build_command", toolchain::BUILD_COMMAND.to_string());
        tokens.insert("verified_on", toolchain::VERIFIED_ON.to_string());

        Self { tokens }
    }

    /// Token names, for error messages.
    fn available(&self) -> String {
        self.tokens.keys().copied().collect::<Vec<_>>().join(", ")
    }
}

/// What [`generate`] produced, for reporting back to the user.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The project root that was created.
    pub root: PathBuf,
    /// Files written, relative to `root`, in path order.
    pub files: Vec<PathBuf>,
    /// Whether rustfmt ran over the generated sources. See [`format_sources`].
    pub formatted: bool,
}

/// Render `template` into `dest`.
///
/// `dest` is created if missing. An existing non-empty directory is refused
/// rather than merged: overwriting a user's work is not recoverable, and a
/// partial merge would produce a project that is neither the template nor what
/// was there before.
///
/// # Errors
/// Returns an error if `dest` is a non-empty directory or a file, if a template
/// references an unknown token, or if writing fails.
pub fn generate(template: &Template, names: &Names, dest: &Path) -> Result<Outcome> {
    ensure_writable(dest)?;

    let context = Context::new(names);
    let files = template.files()?;

    fs::create_dir_all(dest).with_context(|| format!("failed to create {}", dest.display()))?;

    let mut written = Vec::with_capacity(files.len());

    for file in &files {
        let origin = format!("template {}: {}", template.name, file.path.display());
        let rendered = render(file.contents, &context, &origin)?;
        let target = dest.join(&file.path);

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        fs::write(&target, rendered)
            .with_context(|| format!("failed to write {}", target.display()))?;

        written.push(file.path.clone());
    }

    let formatted = format_sources(dest, &written);

    Ok(Outcome {
        root: dest.to_path_buf(),
        files: written,
        formatted,
    })
}

/// Run rustfmt over the generated Rust sources, returning whether it succeeded.
///
/// This is not cosmetic. A project name is substituted into the source, and a
/// longer name makes lines longer, so no fixed template text can be correctly
/// formatted for every possible name. Since the generated CI runs
/// `cargo fmt --check`, an unformatted project would fail its own CI on the
/// first push through no fault of the user.
///
/// A missing or failing rustfmt is not an error: the generated project is still
/// correct and compiles, it may just need one `cargo fmt`. Reporting that is
/// better than refusing to generate.
fn format_sources(root: &Path, files: &[PathBuf]) -> bool {
    let sources: Vec<PathBuf> = files
        .iter()
        .filter(|path| path.extension() == Some(OsStr::new("rs")))
        .map(|path| root.join(path))
        .collect();

    if sources.is_empty() {
        return true;
    }

    Command::new("rustfmt")
        .arg("--edition")
        .arg(toolchain::RUST_EDITION)
        .arg("--quiet")
        .args(&sources)
        .status()
        .is_ok_and(|status| status.success())
}

/// Refuse destinations that already hold something.
fn ensure_writable(dest: &Path) -> Result<()> {
    let Ok(metadata) = fs::metadata(dest) else {
        // Does not exist, which is the normal case.
        return Ok(());
    };

    if !metadata.is_dir() {
        bail!("{} already exists and is not a directory", dest.display());
    }

    let mut entries =
        fs::read_dir(dest).with_context(|| format!("failed to read {}", dest.display()))?;

    if entries.next().is_some() {
        bail!(
            "{} already exists and is not empty; \
             choose a different name or remove the directory first",
            dest.display()
        );
    }

    Ok(())
}

/// Replace every `{% token %}` in `source`.
///
/// `origin` identifies the file in error messages.
///
/// # Errors
/// An unknown or unclosed token is an error rather than a silent pass-through:
/// a typo in a contributed template would otherwise ship a literal `{% crate_nme %}`
/// into a user's source file.
fn render(source: &str, context: &Context, origin: &str) -> Result<String> {
    const OPEN: &str = "{%";
    const CLOSE: &str = "%}";

    let mut out = String::with_capacity(source.len());
    let mut rest = source;

    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after = &rest[start + OPEN.len()..];

        let Some(end) = after.find(CLOSE) else {
            bail!("{origin}: unclosed `{OPEN}` token");
        };

        let key = after[..end].trim();
        let Some(value) = context.tokens.get(key) else {
            bail!(
                "{origin}: unknown token `{key}`; available tokens are: {}",
                context.available()
            );
        };

        out.push_str(value);
        rest = &after[end + CLOSE.len()..];
    }

    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> Context {
        Context::new(&Names::parse("my-token").unwrap())
    }

    #[test]
    fn substitutes_name_forms() {
        let out = render(
            "package = \"{% project_name %}\"\nstruct {% contract_name %};\n{% crate_name %}",
            &context(),
            "test",
        )
        .unwrap();
        assert_eq!(out, "package = \"my-token\"\nstruct MyToken;\nmy_token");
    }

    #[test]
    fn substitutes_verified_toolchain_values() {
        let out = render("{% sdk_version %}|{% rust_target %}", &context(), "test").unwrap();
        assert_eq!(
            out,
            format!(
                "{}|{}",
                toolchain::SOROBAN_SDK_VERSION,
                toolchain::RUST_TARGET
            )
        );
    }

    #[test]
    fn tolerates_missing_inner_whitespace() {
        let out = render("{%project_name%}", &context(), "test").unwrap();
        assert_eq!(out, "my-token");
    }

    #[test]
    fn leaves_github_actions_expressions_untouched() {
        // The reason this delimiter was chosen; a `{{ }}` scanner would break here.
        let source = "runs-on: ${{ matrix.os }}\nkey: ${{ hashFiles('**/Cargo.lock') }}";
        assert_eq!(render(source, &context(), "test").unwrap(), source);
    }

    #[test]
    fn leaves_text_without_tokens_untouched() {
        let source = "fn main() { println!(\"100%\"); }";
        assert_eq!(render(source, &context(), "test").unwrap(), source);
    }

    #[test]
    fn unknown_token_is_an_error_naming_the_file() {
        let err = render("{% crate_nme %}", &context(), "template x: src/lib.rs")
            .unwrap_err()
            .to_string();
        assert!(err.contains("crate_nme"), "{err}");
        assert!(err.contains("src/lib.rs"), "{err}");
        assert!(
            err.contains("crate_name"),
            "should suggest real tokens: {err}"
        );
    }

    #[test]
    fn unclosed_token_is_an_error() {
        assert!(render("{% project_name", &context(), "test").is_err());
    }

    #[test]
    fn refuses_non_empty_destination() {
        let dir = tempfile::tempdir().unwrap();
        let occupied = dir.path().join("taken");
        fs::create_dir(&occupied).unwrap();
        fs::write(occupied.join("keep.txt"), "important").unwrap();

        let err = ensure_writable(&occupied).unwrap_err().to_string();
        assert!(err.contains("not empty"), "{err}");
    }

    #[test]
    fn accepts_missing_or_empty_destination() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ensure_writable(&dir.path().join("new")).is_ok());
        assert!(ensure_writable(dir.path()).is_ok());
    }
}
