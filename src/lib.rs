//! Lumenprint generates known-good Stellar/Soroban smart contract projects.
//!
//! Lumenprint is an ordinary native Rust CLI. It *generates* contract crates;
//! it is not itself a contract and is never compiled to wasm.
//!
//! The library surface exists so integration tests exercise the same code paths
//! the binary does:
//!
//! ```no_run
//! use std::path::Path;
//! use lumenprint::{generate, manifest, naming::Names};
//!
//! # fn main() -> anyhow::Result<()> {
//! let template = manifest::find("minimal")?;
//! let names = Names::parse("my-contract")?;
//! let outcome = generate::generate(&template, &names, Path::new("./my-contract"))?;
//! println!("wrote {} files", outcome.files.len());
//! # Ok(())
//! # }
//! ```
//!
//! # Adding a template
//!
//! Adding a template is a data-only change; see [`manifest`] for the folder
//! layout and [`generate`] for the substitution tokens.

pub mod generate;
pub mod manifest;
pub mod naming;
pub mod toolchain;
