//! The template registry.
//!
//! Templates are discovered from the embedded [`TEMPLATES`] tree rather than
//! being listed in code, which is what makes adding one a data-only change: drop
//! a folder under `src/templates/` containing a `template.toml` and a `files/`
//! directory, and it appears in `lumenprint list` and `lumenprint new` with no
//! edits here.
//!
//! # Layout of a template
//!
//! ```text
//! src/templates/minimal/
//!   template.toml            <- name is the folder; this supplies the description
//!   files/                   <- everything below is copied into the new project
//!     Cargo.toml.tmpl
//!     src/lib.rs.tmpl
//! ```
//!
//! # The shared base layer
//!
//! Files every project needs regardless of template — `.gitignore`,
//! `rust-toolchain.toml`, the CI workflow — live once in `src/templates/_base/`
//! and are laid down before the template's own files, which may override any of
//! them by using the same path. Without this, a fix to the CI workflow would
//! have to be repeated in every template and would inevitably be missed in one.
//!
//! Folders whose name starts with `_` are layers, not templates: they never
//! appear in `lumenprint list` and cannot be passed to `--template`.
//!
//! # Path conventions
//!
//! Two rewrites are applied to each path under `files/`, both of which exist so
//! that template sources can live inside this crate without affecting it:
//!
//! - a trailing `.tmpl` is stripped, so a nested `Cargo.toml` does not look like
//!   a real package to Cargo while the template sits in `src/`
//! - a leading `_` on any component becomes `.`, so a template's `.gitignore` or
//!   `.github/` does not apply to the Lumenprint repository itself
//!
//! Both rewrites happen in [`output_path`].

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context as _, Result};
use include_dir::{include_dir, Dir, DirEntry};
use serde::Deserialize;

/// Every file under `src/templates/`, embedded at compile time.
///
/// Embedding keeps the binary self-contained: generating a project needs no
/// network access, no git, and no files installed alongside the executable.
static TEMPLATES: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/templates");

/// The manifest file each template folder must contain.
const MANIFEST_NAME: &str = "template.toml";

/// The subdirectory holding the files to be generated.
const FILES_DIR: &str = "files";

/// The layer supplying files common to every template.
const BASE_LAYER: &str = "_base";

/// A `template.toml`, as written by a contributor.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    /// One-line summary shown by `lumenprint list`.
    description: String,
    /// Sort position in listings; ties fall back to name order.
    ///
    /// Exists so the listing can lead with the template a newcomer should pick
    /// rather than whichever name sorts first.
    #[serde(default)]
    order: u32,
    /// Whether to include the shared `_base` layer. Defaults to `true`.
    ///
    /// A template that needs a genuinely different project shape can opt out,
    /// but overriding individual files by path is usually the better answer.
    #[serde(default = "default_base")]
    base: bool,
}

/// Templates include the base layer unless they say otherwise.
fn default_base() -> bool {
    true
}

/// A template that can be generated.
#[derive(Debug, Clone)]
pub struct Template {
    /// Folder name, and the value accepted by `--template`.
    pub name: String,
    /// One-line summary shown by `lumenprint list`.
    pub description: String,
    /// Sort position in listings.
    pub order: u32,
    /// The embedded `files/` directory this template generates from.
    files: &'static Dir<'static>,
    /// The shared base layer, unless this template opted out.
    base: Option<&'static Dir<'static>>,
}

/// One file to be written into the generated project.
#[derive(Debug, Clone)]
pub struct TemplateFile {
    /// Path relative to the project root, after the `.tmpl` and `_` rewrites.
    pub path: PathBuf,
    /// Raw contents, before token substitution.
    pub contents: &'static str,
}

impl Template {
    /// Collect this template's files, in stable path order.
    ///
    /// Base-layer files come first, then the template's own, which override any
    /// shared file at the same path.
    ///
    /// # Errors
    /// Returns an error if a template file is not valid UTF-8. Templates are
    /// text; a binary asset would silently corrupt under substitution, so this
    /// is rejected rather than passed through.
    pub fn files(&self) -> Result<Vec<TemplateFile>> {
        // A map keyed by output path gives override-by-path and stable ordering
        // in one step.
        let mut merged: BTreeMap<PathBuf, &'static str> = BTreeMap::new();

        if let Some(base) = self.base {
            collect(base, &mut merged)
                .context("the shared _base layer contains an invalid file")?;
        }

        collect(self.files, &mut merged)
            .with_context(|| format!("template {:?} contains an invalid file", self.name))?;

        Ok(merged
            .into_iter()
            .map(|(path, contents)| TemplateFile { path, contents })
            .collect())
    }
}

/// Load every embedded template, ordered for display.
///
/// # Errors
/// Returns an error if a template folder is missing its manifest or `files/`
/// directory, or if a manifest fails to parse. These are contributor mistakes
/// rather than user errors, and the integration tests load the whole registry so
/// they surface at test time.
pub fn registry() -> Result<Vec<Template>> {
    let base_layer = TEMPLATES.get_dir(Path::new(BASE_LAYER).join(FILES_DIR));
    let mut templates = Vec::new();

    for entry in TEMPLATES.dirs() {
        let name = entry
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("template folder {:?} has no readable name", entry.path()))?
            .to_string();

        // Underscore-prefixed folders are shared layers, not templates.
        if name.starts_with('_') {
            continue;
        }

        let manifest_path = entry.path().join(MANIFEST_NAME);
        let manifest_file = TEMPLATES.get_file(&manifest_path).ok_or_else(|| {
            anyhow!("template {name:?} is missing {MANIFEST_NAME}; every template folder needs one")
        })?;
        let manifest_text = manifest_file
            .contents_utf8()
            .ok_or_else(|| anyhow!("{}: not valid UTF-8", manifest_path.display()))?;
        let manifest: RawManifest = toml::from_str(manifest_text)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;

        let files = TEMPLATES
            .get_dir(entry.path().join(FILES_DIR))
            .ok_or_else(|| anyhow!("template {name:?} is missing a {FILES_DIR}/ directory"))?;

        let base = if manifest.base {
            Some(base_layer.ok_or_else(|| {
                anyhow!(
                    "template {name:?} requests the shared {BASE_LAYER} layer, which is missing"
                )
            })?)
        } else {
            None
        };

        templates.push(Template {
            name,
            description: manifest.description,
            order: manifest.order,
            files,
            base,
        });
    }

    templates.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)));
    Ok(templates)
}

/// Look up a single template by name.
///
/// # Errors
/// Returns an error naming the available templates, so a typo does not require
/// a second command to recover from.
pub fn find(name: &str) -> Result<Template> {
    let templates = registry()?;

    templates
        .iter()
        .find(|t| t.name == name)
        .cloned()
        .ok_or_else(|| {
            let available = templates
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow!("unknown template {name:?}; available templates: {available}")
        })
}

/// Recursively gather files from an embedded directory into `out`.
///
/// Later calls overwrite earlier entries at the same output path, which is how
/// a template overrides a base-layer file.
fn collect(dir: &'static Dir<'static>, out: &mut BTreeMap<PathBuf, &'static str>) -> Result<()> {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => collect(sub, out)?,
            DirEntry::File(file) => {
                let contents = file.contents_utf8().ok_or_else(|| {
                    anyhow!(
                        "{}: not valid UTF-8; templates must be text",
                        file.path().display()
                    )
                })?;

                // Strip the `<layer>/files/` prefix so paths are relative to the
                // generated project root.
                let relative = file.path().components().skip(2).collect::<PathBuf>();

                out.insert(output_path(&relative)?, contents);
            }
        }
    }

    Ok(())
}

/// Apply the template path conventions: strip `.tmpl`, turn a leading `_` into `.`.
///
/// # Errors
/// Returns an error for any path that is absolute or contains `..`, so a
/// malformed template cannot write outside the destination directory.
fn output_path(path: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();

    for component in path.components() {
        let Component::Normal(part) = component else {
            bail!(
                "template path {:?} must be relative and free of `..` components",
                path.display()
            );
        };

        let part = part
            .to_str()
            .ok_or_else(|| anyhow!("template path {:?} is not valid UTF-8", path.display()))?;

        let part = match part.strip_prefix('_') {
            Some(rest) => format!(".{rest}"),
            None => part.to_string(),
        };

        out.push(part);
    }

    // Only the file name carries the `.tmpl` marker.
    if let Some(name) = out.file_name().and_then(|n| n.to_str()) {
        if let Some(stripped) = name.strip_suffix(".tmpl") {
            let stripped = stripped.to_string();
            out.pop();
            out.push(stripped);
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads_and_is_non_empty() {
        let templates = registry().expect("every embedded template must parse");
        assert!(!templates.is_empty(), "no templates were discovered");
    }

    #[test]
    fn every_template_has_a_description_and_files() {
        for template in registry().unwrap() {
            assert!(
                !template.description.trim().is_empty(),
                "template {:?} has an empty description",
                template.name
            );
            let files = template.files().unwrap();
            assert!(
                !files.is_empty(),
                "template {:?} generates no files",
                template.name
            );
        }
    }

    #[test]
    fn layers_are_not_offered_as_templates() {
        assert!(
            !registry().unwrap().iter().any(|t| t.name.starts_with('_')),
            "underscore-prefixed folders are layers and must not be listed"
        );
        assert!(
            find(BASE_LAYER).is_err(),
            "the base layer must not be selectable"
        );
    }

    #[test]
    fn every_template_inherits_the_base_layer_files() {
        for template in registry().unwrap() {
            let paths: Vec<_> = template
                .files()
                .unwrap()
                .into_iter()
                .map(|f| f.path)
                .collect();

            for expected in [
                ".gitignore",
                "rust-toolchain.toml",
                ".github/workflows/ci.yml",
            ] {
                assert!(
                    paths.contains(&PathBuf::from(expected)),
                    "template {:?} is missing base file {expected}",
                    template.name
                );
            }
        }
    }

    #[test]
    fn find_reports_available_templates_on_typo() {
        let err = find("nope").unwrap_err().to_string();
        assert!(
            err.contains("available templates"),
            "unhelpful error: {err}"
        );
        assert!(
            err.contains("minimal"),
            "error should list real templates: {err}"
        );
    }

    #[test]
    fn strips_tmpl_suffix() {
        assert_eq!(
            output_path(Path::new("Cargo.toml.tmpl")).unwrap(),
            Path::new("Cargo.toml")
        );
        assert_eq!(
            output_path(Path::new("src/lib.rs.tmpl")).unwrap(),
            Path::new("src/lib.rs")
        );
    }

    #[test]
    fn rewrites_leading_underscore_to_dot() {
        assert_eq!(
            output_path(Path::new("_gitignore")).unwrap(),
            Path::new(".gitignore")
        );
        assert_eq!(
            output_path(Path::new("_github/workflows/ci.yml")).unwrap(),
            Path::new(".github/workflows/ci.yml")
        );
    }

    #[test]
    fn leaves_ordinary_paths_alone() {
        assert_eq!(
            output_path(Path::new("src/test.rs")).unwrap(),
            Path::new("src/test.rs")
        );
    }

    #[test]
    fn rejects_escaping_paths() {
        assert!(output_path(Path::new("../outside")).is_err());
        assert!(output_path(Path::new("/etc/passwd")).is_err());
    }
}
