//! Command-line entry point.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

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
    List,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::New {
            name,
            template,
            path,
        } => new(&name, &template, path),
        Command::List => list(),
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

/// Print the registry as an aligned table.
fn list() -> Result<()> {
    let templates = manifest::registry()?;

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
