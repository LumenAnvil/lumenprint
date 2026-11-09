//! Command-line entry point.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Serialize;

use lumenprint::{generate, manifest, naming::Names, toolchain};

/// The template used when `--template` is not given.
const DEFAULT_TEMPLATE: &str = "minimal";

#[derive(Debug, Parser)]
#[command(
    name = "lumenprint",
    version,
    about = "Generate known-good Stellar/Soroban smart contract projects",
    long_about = None,
    after_help = concat!(
        "Generated projects target soroban-sdk and the wasm32v1-none target, \
         and are built with `stellar contract build`.\n\
         Run `lumenprint list` to see the available templates."
    )
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate a new Soroban contract project
    New {
        /// Project name; also the directory and Cargo package name
        name: String,

        /// Template to generate from
        #[arg(short, long, default_value = DEFAULT_TEMPLATE)]
        template: String,

        /// Directory to create (defaults to ./<name>)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// List the available templates
    List {
        /// Emit the templates as JSON, for scripts and editor integrations
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::New {
            name,
            template,
            path,
        } => new(&name, &template, path),
        Command::List { json } => list(json),
    }
}

/// Generate a project, then print the next steps a beginner needs.
fn new(name: &str, template_name: &str, path: Option<PathBuf>) -> Result<()> {
    let names = Names::parse(name)?;
    let template = manifest::find(template_name)?;
    let dest = path.unwrap_or_else(|| PathBuf::from(&names.project));

    let outcome = generate::generate(&template, &names, &dest)?;

    println!(
        "Created {} from the `{}` template ({} files)",
        outcome.root.display(),
        template.name,
        outcome.files.len()
    );
    println!(
        "  soroban-sdk {}, target {}",
        toolchain::SOROBAN_SDK_VERSION,
        toolchain::RUST_TARGET
    );
    if !outcome.formatted {
        println!();
        println!("Note: rustfmt was not available, so the sources were left unformatted.");
        println!("      Run `cargo fmt` in the project before pushing, or CI will flag it.");
    }

    println!();
    println!("Next steps:");
    println!("  cd {}", outcome.root.display());
    println!("  cargo test                 # run the generated tests");
    println!("  {}    # build the wasm", toolchain::BUILD_COMMAND);

    Ok(())
}

/// Print the registry, as an aligned table or, with `--json`, as JSON.
fn list(as_json: bool) -> Result<()> {
    let templates = manifest::registry()?;

    if as_json {
        println!("{}", templates_json(&templates)?);
        return Ok(());
    }

    let width = templates
        .iter()
        .map(|t| t.name.len())
        .max()
        .unwrap_or_default();

    for template in &templates {
        println!("{:<width$}  {}", template.name, template.description);
    }

    Ok(())
}

/// One template, as it appears in `lumenprint list --json`.
///
/// A dedicated view type keeps the JSON a deliberate public contract rather than
/// whatever the internal `Template` happens to hold, so the two can evolve
/// independently.
#[derive(Debug, Serialize)]
struct TemplateInfo<'a> {
    /// Folder name, and the value accepted by `--template`.
    name: &'a str,
    /// One-line summary.
    description: &'a str,
    /// Sort position used for the default listing order.
    order: u32,
}

/// Render the registry as a pretty-printed JSON array, in listing order.
fn templates_json(templates: &[manifest::Template]) -> Result<String> {
    let infos: Vec<TemplateInfo> = templates
        .iter()
        .map(|t| TemplateInfo {
            name: &t.name,
            description: &t.description,
            order: t.order,
        })
        .collect();

    Ok(serde_json::to_string_pretty(&infos)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn list_defaults_to_the_table_and_accepts_json() {
        let cli = Cli::try_parse_from(["lumenprint", "list"]).unwrap();
        assert!(matches!(cli.command, Command::List { json: false }));

        let cli = Cli::try_parse_from(["lumenprint", "list", "--json"]).unwrap();
        assert!(matches!(cli.command, Command::List { json: true }));
    }

    #[test]
    fn templates_json_is_valid_and_carries_every_template() {
        let templates = manifest::registry().unwrap();
        let rendered = templates_json(&templates).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let array = parsed.as_array().expect("the document is a JSON array");
        assert_eq!(array.len(), templates.len());

        // Every entry has the three documented keys, and the names round-trip.
        for (entry, template) in array.iter().zip(&templates) {
            assert_eq!(entry["name"], template.name.as_str());
            assert_eq!(entry["description"], template.description.as_str());
            assert!(entry["order"].is_number());
        }

        // The listing order matches the table's (registry order).
        assert_eq!(array[0]["name"], "minimal");
    }
}
